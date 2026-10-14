use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex, OnceLock};

use eggsearch::core::config::AppConfig;
use eggsearch::sdk::{EggsearchClient, SdkError};
use futures::FutureExt;
use pyo3::create_exception;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString, PyTuple};
use serde_json::{Number, Value};
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore};
use tokio_util::sync::CancellationToken;

const MAX_JSON_BYTES: usize = 1_048_576;
const MAX_DEPTH: usize = 64;
const MAX_NODES: usize = 10_000;
const MAX_IN_FLIGHT: usize = 32;

create_exception!(_native, EggsearchError, pyo3::exceptions::PyException);

static RUNTIME_INIT: OnceLock<()> = OnceLock::new();
static IN_FLIGHT: LazyLock<Arc<Semaphore>> =
    LazyLock::new(|| Arc::new(Semaphore::new(MAX_IN_FLIGHT)));

fn initialize_runtime() {
    RUNTIME_INIT.get_or_init(|| {
        let mut builder = tokio::runtime::Builder::new_multi_thread();
        builder
            .worker_threads(4)
            .max_blocking_threads(8)
            .enable_all();
        pyo3_async_runtimes::tokio::init(builder);
    });
}

#[derive(Default)]
struct Lifecycle {
    closed: bool,
    active: usize,
}

struct ClientState {
    client: EggsearchClient,
    lifecycle: Mutex<Lifecycle>,
    cancelled: CancellationToken,
    drained: Notify,
}

impl ClientState {
    fn register(self: &Arc<Self>) -> Result<ActiveCall, SdkError> {
        let mut lifecycle = self
            .lifecycle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if lifecycle.closed {
            return Err(SdkError {
                code: "client_closed".into(),
                message: "Eggsearch client is closed".into(),
                repair: None,
                data: None,
            });
        }
        lifecycle.active += 1;
        Ok(ActiveCall {
            state: self.clone(),
        })
    }

    fn close(&self) {
        let mut lifecycle = self
            .lifecycle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        lifecycle.closed = true;
        drop(lifecycle);
        self.cancelled.cancel();
    }

    async fn wait_drained(&self) {
        loop {
            let notified = self.drained.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let active = self
                .lifecycle
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .active;
            if active == 0 {
                return;
            }
            notified.await;
        }
    }
}

struct ActiveCall {
    state: Arc<ClientState>,
}

impl Drop for ActiveCall {
    fn drop(&mut self) {
        let mut lifecycle = self
            .state
            .lifecycle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        lifecycle.active = lifecycle.active.saturating_sub(1);
        let drained = lifecycle.active == 0;
        drop(lifecycle);
        if drained {
            self.state.drained.notify_waiters();
        }
    }
}

#[pyclass(name = "AsyncEggsearch")]
struct PyAsyncEggsearch {
    state: Arc<ClientState>,
}

#[pymethods]
impl PyAsyncEggsearch {
    #[new]
    #[pyo3(signature = (config_path=None))]
    fn new(config_path: Option<PathBuf>) -> PyResult<Self> {
        let config = match config_path {
            Some(path) => AppConfig::load(&path).map_err(|error| {
                PyValueError::new_err(format!("invalid Eggsearch configuration: {error}"))
            })?,
            None => AppConfig::default(),
        };
        let client = EggsearchClient::new(config).map_err(|error| {
            PyValueError::new_err(format!("invalid Eggsearch configuration: {error}"))
        })?;
        Ok(Self {
            state: Arc::new(ClientState {
                client,
                lifecycle: Mutex::new(Lifecycle::default()),
                cancelled: CancellationToken::new(),
                drained: Notify::new(),
            }),
        })
    }

    fn execute<'py>(
        &self,
        py: Python<'py>,
        tool: String,
        arguments: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        initialize_runtime();
        let active_call = self
            .state
            .register()
            .map_err(|error| sdk_error(py, error))?;
        let arguments = python_to_json(py, arguments)?;
        let permit = IN_FLIGHT.clone().try_acquire_owned().map_err(|_| {
            sdk_error(
                py,
                SdkError {
                    code: "budget_invalid".into(),
                    message: "native execution concurrency limit reached".into(),
                    repair: None,
                    data: None,
                },
            )
        })?;
        let state = self.state.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let _active_call = active_call;
            let _permit: OwnedSemaphorePermit = permit;
            let result = std::panic::AssertUnwindSafe(async {
                tokio::select! {
                    _ = state.cancelled.cancelled() => Err(SdkError {
                        code: "client_closed".into(),
                        message: "Eggsearch client closed while the operation was running".into(),
                        repair: None,
                        data: None,
                    }),
                    result = state.client.execute(tool, arguments) => result,
                }
            })
            .catch_unwind()
            .await
            .unwrap_or_else(|_| {
                Err(SdkError {
                    code: "internal".into(),
                    message: "internal native operation failure".into(),
                    repair: None,
                    data: None,
                })
            });
            Python::attach(|py| match result {
                Ok(value) => json_to_python(py, &value).map(Bound::unbind),
                Err(error) => Err(sdk_error(py, error)),
            })
        })
    }

    fn aclose<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        initialize_runtime();
        self.state.close();
        let state = self.state.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            state.wait_drained().await;
            Python::attach(|py| Ok(py.None()))
        })
    }
}

#[pyfunction]
fn tool_names() -> Vec<&'static str> {
    EggsearchClient::tool_names().collect()
}

#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyAsyncEggsearch>()?;
    module.add("EggsearchError", module.py().get_type::<EggsearchError>())?;
    module.add_function(wrap_pyfunction!(tool_names, module)?)?;
    Ok(())
}

fn python_to_json(py: Python<'_>, object: &Bound<'_, PyAny>) -> PyResult<Value> {
    if !object.is_instance_of::<PyDict>() {
        return Err(PyTypeError::new_err("tool arguments must be a dict"));
    }
    let mut seen = HashSet::new();
    let mut nodes = 0;
    let value = python_value_to_json(object, 0, &mut nodes, &mut seen)?;
    let encoded = serde_json::to_vec(&value)
        .map_err(|_| PyValueError::new_err("tool arguments are not valid JSON values"))?;
    if encoded.len() > MAX_JSON_BYTES {
        return Err(PyValueError::new_err(
            "tool arguments exceed the 1 MiB limit",
        ));
    }
    let _ = py;
    Ok(value)
}

fn python_value_to_json(
    object: &Bound<'_, PyAny>,
    depth: usize,
    nodes: &mut usize,
    seen: &mut HashSet<usize>,
) -> PyResult<Value> {
    *nodes += 1;
    if depth > MAX_DEPTH || *nodes > MAX_NODES {
        return Err(PyValueError::new_err(
            "tool arguments exceed structural limits",
        ));
    }
    if object.is_none() {
        return Ok(Value::Null);
    }
    if object.is_instance_of::<PyBool>() {
        return object.extract::<bool>().map(Value::Bool);
    }
    if object.is_instance_of::<PyInt>() {
        if let Ok(value) = object.extract::<i64>() {
            return Ok(Value::Number(Number::from(value)));
        }
        if let Ok(value) = object.extract::<u64>() {
            return Ok(Value::Number(Number::from(value)));
        }
        return Err(PyValueError::new_err(
            "integer is outside the JSON numeric range",
        ));
    }
    if object.is_instance_of::<PyFloat>() {
        let value = object.extract::<f64>()?;
        let number = Number::from_f64(value)
            .ok_or_else(|| PyValueError::new_err("non-finite numbers are not valid JSON"))?;
        return Ok(Value::Number(number));
    }
    if object.is_instance_of::<PyString>() {
        return object.extract::<String>().map(Value::String);
    }

    let identity = object.as_ptr() as usize;
    if !seen.insert(identity) {
        return Err(PyValueError::new_err(
            "cyclic tool arguments are not supported",
        ));
    }
    let converted = if let Ok(dict) = object.cast::<PyDict>() {
        if dict.len() > MAX_NODES {
            Err(PyValueError::new_err(
                "tool arguments exceed structural limits",
            ))
        } else {
            let mut result = serde_json::Map::new();
            for (key, value) in dict.iter() {
                let key = key.extract::<String>().map_err(|_| {
                    PyTypeError::new_err("tool argument object keys must be strings")
                })?;
                let value = python_value_to_json(&value, depth + 1, nodes, seen)?;
                result.insert(key, value);
            }
            Ok(Value::Object(result))
        }
    } else if let Ok(list) = object.cast::<PyList>() {
        if list.len() > MAX_NODES {
            Err(PyValueError::new_err(
                "tool arguments exceed structural limits",
            ))
        } else {
            let mut result = Vec::with_capacity(list.len());
            for value in list.iter() {
                result.push(python_value_to_json(&value, depth + 1, nodes, seen)?);
            }
            Ok(Value::Array(result))
        }
    } else if let Ok(tuple) = object.cast::<PyTuple>() {
        if tuple.len() > MAX_NODES {
            Err(PyValueError::new_err(
                "tool arguments exceed structural limits",
            ))
        } else {
            let mut result = Vec::with_capacity(tuple.len());
            for value in tuple.iter() {
                result.push(python_value_to_json(&value, depth + 1, nodes, seen)?);
            }
            Ok(Value::Array(result))
        }
    } else {
        Err(PyTypeError::new_err(
            "tool arguments must contain only JSON-compatible values",
        ))
    };
    seen.remove(&identity);
    converted
}

fn json_to_python<'py>(py: Python<'py>, value: &Value) -> PyResult<Bound<'py, PyAny>> {
    match value {
        Value::Null => Ok(py.None().into_bound(py)),
        Value::Bool(value) => Ok(value.into_pyobject(py)?.to_owned().into_any()),
        Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                Ok(value.into_pyobject(py)?.into_any())
            } else if let Some(value) = value.as_u64() {
                Ok(value.into_pyobject(py)?.into_any())
            } else {
                Ok(value
                    .as_f64()
                    .unwrap_or_default()
                    .into_pyobject(py)?
                    .into_any())
            }
        }
        Value::String(value) => Ok(value.into_pyobject(py)?.into_any()),
        Value::Array(values) => {
            let list = PyList::empty(py);
            for value in values {
                list.append(json_to_python(py, value)?)?;
            }
            Ok(list.into_any())
        }
        Value::Object(values) => {
            let dict = PyDict::new(py);
            for (key, value) in values {
                dict.set_item(key, json_to_python(py, value)?)?;
            }
            Ok(dict.into_any())
        }
    }
}

fn sdk_error(py: Python<'_>, error: SdkError) -> PyErr {
    let exception = PyErr::new::<EggsearchError, _>(error.message.clone());
    let value = exception.value(py);
    let _ = value.setattr("code", error.code);
    let _ = value.setattr("message", error.message);
    let repair = error
        .repair
        .as_ref()
        .map(|value| json_to_python(py, value))
        .transpose();
    let data = error
        .data
        .as_ref()
        .map(|value| json_to_python(py, value))
        .transpose();
    if let Ok(repair) = repair {
        let _ = value.setattr("repair", repair);
    }
    if let Ok(data) = data {
        let _ = value.setattr("data", data);
    }
    exception
}
