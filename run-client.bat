@echo off
rem Runs the Canastra client against an H5 client folder.
rem Usage: run-client.bat ["<H5 client folder>"]
rem To log in, set CANASTRA_LOGIN_KEY to the login server's noise_public key (and CANASTRA_LOGIN
rem to its host:port when it is not 127.0.0.1:2106) before running.
setlocal
cd /d "%~dp0"
set "CLIENT=%~1"
if "%CLIENT%"=="" set "CLIENT=%USERPROFILE%\Documents\Lineage II - The Chaotic Throne - Freya - High Five"
cargo run --release -p canastra-client -- "%CLIENT%"
if errorlevel 1 pause
