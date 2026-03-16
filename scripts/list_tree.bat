@echo off
setlocal EnableExtensions EnableDelayedExpansion

REM list_tree.bat
REM Creates a complete inventory of files/folders for the Humanity repo.
REM Outputs to: tools\inventory\inventory_<YYYY-MM-DD>_<HHMMSS>.txt
REM Also writes a stable latest copy: tools\inventory\inventory_latest.txt

REM --- Resolve repo root as the folder containing this script's parent (tools\) ---
REM Recommended placement: Humanity\tools\list_tree.bat
set "SCRIPT_DIR=%~dp0"
for %%I in ("%SCRIPT_DIR%..") do set "REPO_ROOT=%%~fI"

REM --- Output folder ---
set "OUT_DIR=%REPO_ROOT%\tools\inventory"
if not exist "%OUT_DIR%" mkdir "%OUT_DIR%"

REM --- Timestamp (locale-safe-ish). Uses WMIC if available; falls back to DATE/TIME. ---
set "TS="
for /f "tokens=2 delims==" %%A in ('wmic os get localdatetime /value 2^>nul') do set "TS=%%A"
if defined TS (
  set "YYYY=%TS:~0,4%"
  set "MM=%TS:~4,2%"
  set "DD=%TS:~6,2%"
  set "hh=%TS:~8,2%"
  set "mn=%TS:~10,2%"
  set "ss=%TS:~12,2%"
  set "STAMP=%YYYY%-%MM%-%DD%_%hh%%mn%%ss%"
) else (
  REM Fallback (may vary by locale)
  set "STAMP=%DATE%_%TIME%"
  set "STAMP=%STAMP:/=-%"
  set "STAMP=%STAMP::=-%"
  set "STAMP=%STAMP: =%"
  set "STAMP=%STAMP:.=%"
)

set "OUT_FILE=%OUT_DIR%\inventory_%STAMP%.txt"
set "OUT_LATEST=%OUT_DIR%\inventory_latest.txt"

REM --- Write header ---
(
  echo Humanity Repository Inventory
  echo Root: %REPO_ROOT%
  echo Generated: %DATE% %TIME%
  echo.
  echo ===== DIRECTORY TREE (folders) =====
) > "%OUT_FILE%"

REM --- Folder tree (folders only) ---
pushd "%REPO_ROOT%"
tree /A /F | findstr /V /R /C:"^\s*$" >> "%OUT_FILE%"
echo.>>"%OUT_FILE%"

REM --- Full file list with sizes and modified times ---
(
  echo ===== FILE LIST (recursive) =====
  echo Format: RelativePath ^| SizeBytes ^| ModifiedDateTime
) >> "%OUT_FILE%"

for /f "delims=" %%F in ('dir /a:-d /b /s 2^>nul') do (
  set "FULL=%%F"
  set "REL=!FULL:%REPO_ROOT%\=!"
  for %%S in ("%%F") do (
    set "SIZE=%%~zS"
    set "MOD=%%~tS"
  )
  echo !REL! ^| !SIZE! ^| !MOD!>> "%OUT_FILE%"
)

REM --- Stable copy ---
copy /y "%OUT_FILE%" "%OUT_LATEST%" >nul

popd

echo Wrote:
echo   %OUT_FILE%
echo   %OUT_LATEST%
endlocal
exit /b 0
