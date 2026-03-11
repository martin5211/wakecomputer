@echo off
setlocal

:: Require administrator
net session >nul 2>&1
if %errorlevel% neq 0 (
    echo ERROR: Run this script as Administrator.
    exit /b 1
)

set SERVICE_NAME=wakecomputer-agent
set INSTALL_DIR=%ProgramFiles%\wakecomputer-agent
set EXE_NAME=wakecomputer-agent.exe
set PORT=9877

:: Prompt for token (same as agent_token in config.yaml)
if "%~1"=="" (
    set /p TOKEN="Enter agent token: "
) else (
    set TOKEN=%~1
)

if "%TOKEN%"=="" (
    echo ERROR: Token is required.
    exit /b 1
)

:: Stop existing service if running
sc query %SERVICE_NAME% >nul 2>&1
if %errorlevel% equ 0 (
    echo Stopping existing service...
    sc stop %SERVICE_NAME% >nul 2>&1
    timeout /t 2 /nobreak >nul
    sc delete %SERVICE_NAME%
    timeout /t 1 /nobreak >nul
)

:: Copy binary
if not exist "%INSTALL_DIR%" mkdir "%INSTALL_DIR%"

set "BIN="
if exist "%~dp0%EXE_NAME%" set "BIN=%~dp0%EXE_NAME%"
if "%BIN%"=="" for /d %%d in ("%~dp0..\target\*") do (
    if exist "%%d\release\%EXE_NAME%" set "BIN=%%d\release\%EXE_NAME%"
)
if "%BIN%"=="" if exist "%~dp0..\target\release\%EXE_NAME%" set "BIN=%~dp0..\target\release\%EXE_NAME%"

if "%BIN%"=="" (
    echo ERROR: Cannot find %EXE_NAME%.
    echo Place it next to this script or build with: cargo build -p wakecomputer-agent --release
    exit /b 1
)

copy /y "%BIN%" "%INSTALL_DIR%\%EXE_NAME%" >nul

:: Create service
sc create %SERVICE_NAME% ^
    binPath= "\"%INSTALL_DIR%\%EXE_NAME%\" --token %TOKEN% --listen 0.0.0.0:%PORT%" ^
    start= auto ^
    DisplayName= "WakeComputer Agent"

if %errorlevel% neq 0 (
    echo ERROR: Failed to create service.
    exit /b 1
)

sc description %SERVICE_NAME% "Shutdown agent for wakecomputer - accepts remote shutdown commands"

:: Firewall rule
netsh advfirewall firewall show rule name="%SERVICE_NAME%" >nul 2>&1
if %errorlevel% neq 0 (
    netsh advfirewall firewall add rule ^
        name="%SERVICE_NAME%" ^
        dir=in action=allow protocol=TCP localport=%PORT%
)

:: Start service
sc start %SERVICE_NAME%

echo.
echo Installed and started %SERVICE_NAME%
echo   Binary:  %INSTALL_DIR%\%EXE_NAME%
echo   Port:    %PORT%
echo   Service: %SERVICE_NAME%
