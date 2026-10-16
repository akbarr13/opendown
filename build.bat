@echo off
rem Build OpenDown (Rust, x86_64-pc-windows-gnu via portable w64devkit).
rem Adjust the toolchain folder below if yours lives elsewhere.
set PATH=%USERPROFILE%\tools\w64devkit\bin;%USERPROFILE%\.cargo\bin;%PATH%
cd /d "%~dp0"
cargo build --release %*
pause
