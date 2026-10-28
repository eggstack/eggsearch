import argparse
import hashlib
import json
import tarfile
import zipfile
from email.parser import Parser
from pathlib import Path

from packaging.specifiers import SpecifierSet
from packaging.utils import parse_sdist_filename, parse_wheel_filename


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    parser.add_argument("version")
    parser.add_argument("manifest", type=Path)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--qualification-run-id", required=True)
    args = parser.parse_args()

    wheels = sorted(args.directory.rglob("*.whl"))
    sdists = sorted(args.directory.rglob("*.tar.gz"))
    if len(wheels) != 6 or len(sdists) != 1:
        raise SystemExit(f"expected six wheels and one sdist, found {len(wheels)} and {len(sdists)}")

    required_platforms = {
        "x86_64": ("manylinux2014_x86_64", "manylinux_2_17_x86_64"),
        "aarch64": ("manylinux2014_aarch64", "manylinux_2_17_aarch64"),
        "macos_x86_64": ("macosx_10_12_x86_64",),
        "macos_arm64": ("macosx_11_0_arm64", "macosx_12_0_arm64"),
        "win_amd64": ("win_amd64",),
        "armv7l": ("manylinux_2_28_armv7l",),
    }
    found = set()
    records = []
    for wheel in wheels:
        name, version, _, tags = parse_wheel_filename(wheel.name)
        if name != "eggsearch" or str(version) != args.version:
            raise SystemExit(f"unexpected distribution or version in {wheel.name}")
        if not all(tag.interpreter == "cp311" and tag.abi == "abi3" for tag in tags):
            raise SystemExit(f"unexpected interpreter ABI tags in {wheel.name}")
        platforms = {tag.platform for tag in tags}
        platform_id = next(
            (
                key
                for key, expected in required_platforms.items()
                if any(platform.startswith(prefix) for platform in platforms for prefix in expected)
            ),
            None,
        )
        if platform_id is None or platform_id in found:
            raise SystemExit(f"unexpected or duplicate wheel platform in {wheel.name}: {platforms}")
        found.add(platform_id)
        with zipfile.ZipFile(wheel) as archive:
            files = set(archive.namelist())
            package_files = {
                path
                for path in files
                if path.startswith("eggsearch/")
            }
            dist_info_roots = {
                path.split("/", 1)[0]
                for path in files
                if ".dist-info/" in path
            }
            if len(dist_info_roots) != 1:
                raise SystemExit(f"unexpected wheel metadata roots in {wheel.name}")
            native_files = {
                path
                for path in package_files
                if path.startswith("eggsearch/_native.") and path.endswith((".so", ".pyd"))
            }
            required_package_files = {
                "eggsearch/__init__.py",
                "eggsearch/_native.pyi",
                "eggsearch/py.typed",
                "eggsearch/types.py",
            }
            if not required_package_files.issubset(package_files):
                raise SystemExit(f"missing Python package files in {wheel.name}")
            if len(native_files) != 1:
                raise SystemExit(f"expected one native extension in {wheel.name}")
            if package_files != required_package_files | native_files:
                raise SystemExit(f"unexpected Python package contents in {wheel.name}")
            if any(Path(path).name in {"eggsearch", "eggsearch.exe"} for path in files):
                raise SystemExit(f"unexpected CLI executable in {wheel.name}")
            dist_info_prefix = f"{next(iter(dist_info_roots))}/"
            dist_info_files = {path for path in files if path.startswith(dist_info_prefix)}
            required_dist_info = {
                f"{dist_info_prefix}METADATA",
                f"{dist_info_prefix}WHEEL",
                f"{dist_info_prefix}RECORD",
                f"{dist_info_prefix}licenses/LICENSE-PYTHON",
            }
            if not required_dist_info.issubset(dist_info_files):
                raise SystemExit(f"missing license or wheel metadata in {wheel.name}")
            allowed_dist_info = required_dist_info | {
                f"{dist_info_prefix}sboms/eggsearch-python.cyclonedx.json"
            }
            if not dist_info_files.issubset(allowed_dist_info):
                raise SystemExit(f"unexpected wheel metadata contents in {wheel.name}")
            if files != package_files | dist_info_files:
                raise SystemExit(f"unexpected top-level wheel contents in {wheel.name}")
            metadata_path = f"{dist_info_prefix}METADATA"
            metadata = Parser().parsestr(archive.read(metadata_path).decode())
            if metadata.get("Name") != "eggsearch" or metadata.get("Version") != args.version:
                raise SystemExit(f"wheel metadata mismatch in {wheel.name}")
            if SpecifierSet(metadata.get("Requires-Python", "")) != SpecifierSet(">=3.11,<3.16"):
                raise SystemExit(f"unexpected Requires-Python in {wheel.name}")
            if metadata.get("License-Expression") != "MIT":
                raise SystemExit(f"unexpected wheel license in {wheel.name}")
            if metadata.get("Description-Content-Type", "").split(";", 1)[0] != "text/markdown":
                raise SystemExit(f"wheel is missing the Markdown project README in {wheel.name}")
            if len(metadata.get_payload()) < 300:
                raise SystemExit(f"wheel project README is unexpectedly short in {wheel.name}")
        records.append({"file": wheel.name, "sha256": hashlib.sha256(wheel.read_bytes()).hexdigest()})

    if found != set(required_platforms):
        raise SystemExit(f"wheel platform inventory mismatch: {found}")
    audits = sorted(args.directory.rglob("armv7-audit.txt"))
    if len(audits) != 1:
        raise SystemExit(f"expected one ARMv7 ELF audit record, found {len(audits)}")
    audit = audits[0].read_text(encoding="utf-8")
    required_audit_lines = {
        "elf_class=ELF32",
        "machine=ARM",
        "abi=hard-float",
        "tag=manylinux_2_28_armv7l",
    }
    if not required_audit_lines.issubset(set(audit.splitlines())):
        raise SystemExit("ARMv7 ELF audit record is incomplete")
    glibc_line = next((line for line in audit.splitlines() if line.startswith("max_glibc=")), None)
    if glibc_line is None:
        raise SystemExit("ARMv7 ELF audit record is missing its glibc floor")
    if not any(line.startswith("dynamic_dependencies=") for line in audit.splitlines()):
        raise SystemExit("ARMv7 ELF audit record is missing dynamic dependencies")
    glibc = tuple(map(int, glibc_line.split("=", 1)[1].split(".")))
    if glibc > (2, 28):
        raise SystemExit(f"ARMv7 wheel exceeds its manylinux_2_28 floor: {glibc}")
    audit_record = {
        "file": str(audits[0].relative_to(args.directory)),
        "sha256": hashlib.sha256(audits[0].read_bytes()).hexdigest(),
    }
    sdist_name, sdist_version = parse_sdist_filename(sdists[0].name)
    if sdist_name != "eggsearch" or str(sdist_version) != args.version:
        raise SystemExit(f"unexpected source distribution: {sdists[0].name}")
    with tarfile.open(sdists[0], "r:gz") as archive:
        members = archive.getmembers()
        names = {member.name for member in members}
        roots = {name.split("/", 1)[0] for name in names}
        if len(roots) != 1:
            raise SystemExit("sdist contains files outside its single distribution root")
        for member in members:
            path = Path(member.name)
            if path.is_absolute() or ".." in path.parts:
                raise SystemExit(f"unsafe path in sdist: {member.name}")
            if path.suffix in {".so", ".pyd", ".pyc", ".dll", ".dylib"}:
                raise SystemExit(f"unexpected compiled artifact in sdist: {member.name}")
            if path.suffix.lower() in {".log", ".ansi", ".cast", ".dump", ".pem", ".key", ".p12"}:
                raise SystemExit(f"unexpected transcript, dump, or credential in sdist: {member.name}")
            if member.isfile() and member.mode & 0o111:
                raise SystemExit(f"unexpected executable file in sdist: {member.name}")
            if any(part in {"target", ".git", "__pycache__"} for part in path.parts):
                raise SystemExit(f"unexpected generated directory in sdist: {member.name}")
            if path.name.lower() in {".env", "id_rsa", "id_ed25519", "credentials.json"} or path.name.lower().startswith(".env."):
                raise SystemExit(f"unexpected credential-like file in sdist: {member.name}")
        if not any(name.endswith("/Cargo.lock") and "bindings/python" not in name for name in names):
            raise SystemExit("sdist is missing the root Cargo.lock")
        if not any(name.endswith("/bindings/python/Cargo.lock") for name in names):
            raise SystemExit("sdist is missing bindings/python/Cargo.lock")
        if not any(name.endswith("/LICENSE") for name in names):
            raise SystemExit("sdist is missing its project license")
        if not any(name.endswith("/README.md") for name in names):
            raise SystemExit("sdist is missing the root project README")
        if not any(name.endswith("/bindings/python/PYTHON_README.md") for name in names):
            raise SystemExit("sdist is missing the Python package README")
        metadata_member = next((member for member in members if member.name.endswith("/PKG-INFO")), None)
        if metadata_member is None:
            raise SystemExit("sdist is missing PKG-INFO")
        metadata = Parser().parsestr(archive.extractfile(metadata_member).read().decode())
        if metadata.get("Name") != "eggsearch" or metadata.get("Version") != args.version:
            raise SystemExit("sdist metadata mismatch")
        if SpecifierSet(metadata.get("Requires-Python", "")) != SpecifierSet(">=3.11,<3.16"):
            raise SystemExit("sdist has unexpected Requires-Python metadata")
        if metadata.get("License-Expression") != "MIT" or len(metadata.get_payload()) < 300:
            raise SystemExit("sdist is missing its expected license or project README")
    records.append({"file": sdists[0].name, "sha256": hashlib.sha256(sdists[0].read_bytes()).hexdigest()})
    args.manifest.write_text(
        json.dumps(
            {
                "version": args.version,
                "source_sha": args.source_sha,
                "qualification_run_id": args.qualification_run_id,
                "artifacts": records,
                "armv7_audit": audit_record,
            },
            indent=2,
        )
        + "\n"
    )


if __name__ == "__main__":
    main()
