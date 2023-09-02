workspace "lang"
    architecture "x64"
    configurations { "debug", "release", "dist" }
    startproject "compiler"
	
outputdir = "%{cfg.buildcfg}-%{cfg.system}-%{cfg.architecture}"

include "compiler"
