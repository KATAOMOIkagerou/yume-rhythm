@echo off
chcp 65001 >nul
if not exist "%~dp0bin\桌宠节奏师.exe" (
  echo 请先在本目录运行 npm install 和 npm run build。
  pause
  exit /b 1
)
start "" "%~dp0bin\桌宠节奏师.exe"
