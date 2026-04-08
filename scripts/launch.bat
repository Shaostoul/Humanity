@echo off
REM HumanityOS Launcher — pin this to your taskbar
REM Finds and runs the latest versioned exe in C:\Humanity\
REM No need to update this file when new versions are built.

setlocal enabledelayedexpansion

set "BINDIR=C:\Humanity"
set "LATEST="

if not exist "%BINDIR%" (
    echo ERROR: %BINDIR% does not exist.
    echo Run 'just build-game' first to create a versioned build.
    pause
    exit /b 1
)

REM Find the most recently modified exe matching v*_HumanityOS.exe
for /f "delims=" %%f in ('dir /b /o-d "%BINDIR%\v*_HumanityOS.exe" 2^>nul') do (
    if not defined LATEST set "LATEST=%%f"
)

if not defined LATEST (
    echo ERROR: No HumanityOS builds found in %BINDIR%
    echo Run 'just build-game' first.
    pause
    exit /b 1
)

echo Launching %LATEST%
start "" "%BINDIR%\%LATEST%"
