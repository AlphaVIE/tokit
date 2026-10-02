@echo off
setlocal
where py >nul 2>nul
if %errorlevel%==0 (
	py -3.10 "%~dp0benchmark_matrix.py" %*
	exit /b %errorlevel%
)
python "%~dp0benchmark_matrix.py" %*
