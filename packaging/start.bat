@echo off
rem Starts quaesito for a quiz night (double-click). Phones on the same Wi-Fi can join.
rem Leave this window open while you play; close it to stop.
cd /d "%~dp0"
if not defined IP set IP=0.0.0.0
if not defined PORT set PORT=8080
if not defined QUAESITO_DATA_DIR set QUAESITO_DATA_DIR=%~dp0data
echo Starting quaesito on http://localhost:%PORT%
echo Your quizzes and pictures are kept in %QUAESITO_DATA_DIR%
start "" "http://localhost:%PORT%"
server.exe
pause
