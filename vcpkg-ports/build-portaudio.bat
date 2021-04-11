setlocal
cd /d %~dp0

%VCPKG_ROOT%\vcpkg --overlay-ports=%CD% install portaudio:x64-windows-static-md