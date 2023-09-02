@echo off
pushd ..
setup\premake\premake5.exe --version
setup\premake\premake5.exe vs2022
popd
pause
