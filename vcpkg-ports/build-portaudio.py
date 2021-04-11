import os
import sys
import subprocess


def main():
    vcpkg_root = os.getenv("VCPKG_ROOT")
    if not vcpkg_root:
        sys.stderr.write("VCPKG_ROOT is not set. Is vcpkg installed?")
        return 1
    subprocess.check_call(
        [
            os.path.join(vcpkg_root, "vcpkg"),
            "--overlay-ports=%s" % os.getcwd(),
            "install",
            "portaudio:x64-windows-static-md",
        ]
    )
    return 0


if __name__ == "__main__":
    sys.exit(main() or 0)
