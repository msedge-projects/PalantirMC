@echo off
rem Run tools\progress.py with an interpreter that exists, and keep the window
rem open when this was a double-click.
rem
rem Why this exists: on this machine `python` in a plain command prompt or
rem PowerShell is the Microsoft Store alias. It prints "Python was not found;
rem run without arguments to install from the Microsoft Store" and exits --
rem which a double-click of progress.py shows as a console that opens and
rem closes before anything can be read. So this finds an interpreter in the
rem order that survives that alias, and holds the window open only when
rem nothing but a double-click could have started it.
rem
rem   tools\progress.cmd              the table, and a pause so it can be read
rem   tools\progress.cmd --check      silent, exit 1 on drift -- no pause
rem   tools\progress.cmd --watch      the table, redrawn whenever a document moves
rem   tools\progress.cmd --no-pause   the table without the pause
rem   tools\progress.cmd --which      print the interpreter this would use
rem
rem Every other argument goes to progress.py unchanged.
setlocal EnableExtensions
rem Captured before the argument loop: `shift` moves the numbered parameters,
rem and these have to survive it.
set "ROOT=%~dp0.."
set "SCRIPT=%~dp0progress.py"

rem 1. The py launcher: it knows every install python.org made, needs no PATH
rem    and is the same interpreter `py` at a prompt would pick.
set "PYRUN="
if exist "%LocalAppData%\Programs\Python\Launcher\py.exe" set PYRUN="%LocalAppData%\Programs\Python\Launcher\py.exe" -3

rem 2. An install directly, newest last under the name order (`Python313` after
rem    `Python312`). `dir` cannot wildcard a directory component, so the loop
rem    is over directories and each one's python.exe is checked.
if not defined PYRUN (
  for /d %%D in ("%LocalAppData%\Programs\Python\Python3*") do (
    if exist "%%D\python.exe" set PYRUN="%%D\python.exe"
  )
)

rem Deliberately *not* a third step over `python` on PATH: on this machine that
rem name is the Store alias, which exists, answers "not found" and is useless.
rem The two above cover every real install python.org makes.

if "%~1"=="--which" (
  if defined PYRUN (echo %PYRUN%) else (echo none)
  exit /b 0
)

if not defined PYRUN (
  echo.
  echo No Python interpreter was found. Install one from python.org, or run
  echo this from a shell that already has one.
  echo.
  pause
  exit /b 2
)

rem Arguments one at a time, because `--no-pause` is this wrapper's own flag
rem and must not reach the script. The one-liner that looks equivalent --
rem `set "ARGS=%ARGS:--no-pause=%"` -- is not: with no arguments ARGS is
rem undefined, and cmd answers an undefined variable's substitution with
rem `--no-pause=` itself, so a bare double-click handed the script
rem "unrecognized arguments: --no-pause=". The flag also decides the pause
rem here rather than by another substitution on what may be an empty string.
set "ARGS="
set "PAUSE=1"

:args
if "%~1"=="" goto args_done
if /i "%~1"=="--check" set "PAUSE=0"
if /i "%~1"=="--watch" set "PAUSE=0"
if /i "%~1"=="--no-pause" set "PAUSE=0"
if /i "%~1"=="--no-pause" goto args_next
set "ARGS=%ARGS% %1"
:args_next
shift
goto args

:args_done
rem The repository is the working directory, whatever the caller's is.
pushd "%ROOT%"
%PYRUN% "%SCRIPT%" %ARGS%
set "CODE=%ERRORLEVEL%"
popd

rem A double-click is the one caller that cannot read a console that closes, so
rem it is the one caller that gets a pause. `--check` and `--watch` say
rem "script" and never pause: an agent running the first would have nothing to
rem press, and the second is a pane it stops with Ctrl-C.
if "%PAUSE%"=="0" exit /b %CODE%
echo.
echo (exit %CODE%)
pause
exit /b %CODE%
