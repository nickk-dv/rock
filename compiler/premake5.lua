project "compiler"
	kind "ConsoleApp"
	language "C++"
	cppdialect "C++20"
	staticruntime "off"
	
	files { "src/**.cppm", "src/**.cpp", "src/**.h" }
	includedirs { "src", "llvm" }
	libdirs { "llvm" }
	links { "LLVM-C" }
	targetdir ("../bin/" .. outputdir .. "/%{prj.name}")
	objdir ("../bin-int/" .. outputdir .. "/%{prj.name}")
	
	filter "system:windows"
		systemversion "latest"

	filter "configurations:debug"
		runtime "Debug"
		optimize "Off"
		symbols "On"

	filter "configurations:dist"
		runtime "Release"
		optimize "Full"
		symbols "Off"
