@echo off
setlocal
py -3 "%~dp0redline.py" %*
exit /b %ERRORLEVEL%
