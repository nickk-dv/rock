workspace "lang"
    architecture "x64"
    configurations { "debug", "dist" }
    startproject "compiler"

outputdir = "%{cfg.buildcfg}-%{cfg.system}-%{cfg.architecture}"

include "compiler"
