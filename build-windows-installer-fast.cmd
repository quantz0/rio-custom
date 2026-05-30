@echo off
setlocal

powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0build-windows-installer.ps1" -Dev %*
set "EXITCODE=%ERRORLEVEL%"

echo.
if "%NO_PAUSE%"=="" pause
exit /b %EXITCODE%
