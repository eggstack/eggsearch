import argparse
import re
import subprocess
import tempfile
import zipfile
from pathlib import Path

from packaging.utils import parse_wheel_filename


def output(*command):
    return subprocess.run(command, check=True, capture_output=True, text=True).stdout


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("wheel", type=Path)
    args = parser.parse_args()

    name, _, _, tags = parse_wheel_filename(args.wheel.name)
    if name != "eggsearch" or not tags:
        raise SystemExit("unexpected wheel distribution")
    if not all(tag.interpreter == "cp311" and tag.abi == "abi3" for tag in tags):
        raise SystemExit("ARMv7 wheel must use the cp311 abi3 stable ABI")
    platforms = {tag.platform for tag in tags}
    if platforms != {"manylinux_2_28_armv7l"}:
        raise SystemExit(f"unexpected ARMv7 wheel platform tags: {sorted(platforms)}")

    with zipfile.ZipFile(args.wheel) as archive:
        native = [
            path
            for path in archive.namelist()
            if path.startswith("eggsearch/_native.") and path.endswith(".so")
        ]
        if len(native) != 1:
            raise SystemExit("wheel must contain exactly one native shared object")
        if any(Path(path).name in {"eggsearch", "eggsearch.exe"} for path in archive.namelist()):
            raise SystemExit("wheel unexpectedly bundles the Eggsearch executable")
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / "_native.so"
            binary.write_bytes(archive.read(native[0]))
            header = output("readelf", "-h", str(binary))
            attributes = output("readelf", "-A", str(binary))
            dynamic = output("readelf", "-d", str(binary))
            versions = output("readelf", "--version-info", str(binary))

    if "ELF32" not in header or "Machine:" not in header or "ARM" not in header:
        raise SystemExit("native extension is not a 32-bit ARM ELF")
    if "Type:" not in header or "DYN" not in header or "Version5 EABI" not in header:
        raise SystemExit("native extension is not a dynamically linked ARM EABI5 shared object")
    if "Tag_ABI_VFP_args: VFP registers" not in attributes:
        raise SystemExit("native extension does not declare the ARM hard-float calling convention")
    if "libpython" in dynamic:
        raise SystemExit("native extension unexpectedly links to libpython")
    needed = sorted(set(re.findall(r"Shared library: \[(.*?)\]", dynamic)))
    glibc = [tuple(map(int, value.split("."))) for value in re.findall(r"GLIBC_(\d+(?:\.\d+)+)", versions)]
    if not glibc:
        raise SystemExit("native extension has no inspectable GLIBC symbol requirements")
    maximum = max(glibc)
    if maximum > (2, 28):
        raise SystemExit(f"native extension requires GLIBC {'.'.join(map(str, maximum))}, above 2.28")

    print(f"wheel={args.wheel.name}")
    print("elf_class=ELF32")
    print("machine=ARM")
    print("abi=hard-float")
    print("dynamic_dependencies=" + ",".join(needed))
    print(f"max_glibc={'.'.join(map(str, maximum))}")
    print("tag=manylinux_2_28_armv7l")


if __name__ == "__main__":
    main()
