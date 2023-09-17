@echo off
pushd ..
scripts\premake\premake5.exe --version
scripts\premake\premake5.exe vs2022
popd
pause
