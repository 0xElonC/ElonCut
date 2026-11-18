@echo off
rem Installs what is missing and starts ElonCut. See scripts\dev.ps1.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\dev.ps1" %*
