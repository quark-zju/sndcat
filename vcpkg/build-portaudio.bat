@echo off

setlocal
cd /d %~dp0

if not defined VCPKG_ROOT echo vcpkg seems missing?

%VCPKG_ROOT%\vcpkg ^
  --overlay-ports=%CD%\ports install portaudio:x64-windows-static-md ^
  --x-buildtrees-root=%CD%\buildtrees ^
  --x-install-root=%CD%\installed ^
  --x-packages-root=%CD%\packages ^
  --vcpkg-root=%VCPKG_ROOT%
