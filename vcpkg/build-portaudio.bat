@echo off

setlocal
cd /d %~dp0

if not defined VCPKG_ROOT if defined VCPKG_INSTALLATION_ROOT set VCPKG_ROOT=%VCPKG_INSTALLATION_ROOT%
if not defined VCPKG_ROOT echo vcpkg seems missing?

%VCPKG_ROOT%\vcpkg --overlay-ports=%CD%\ports install portaudio:x64-windows-static-md
