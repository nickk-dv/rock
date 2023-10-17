project "compiler"
	kind "ConsoleApp"
	language "C++"
	cppdialect "C++17"
	staticruntime "off"
	
	files { "src/**.cpp", "src/**.h" }
	
	includedirs
	{
		"src",
		"llvm"
	}
	
	libdirs { "llvm" }
	links { "LLVM-C" }
	
	targetdir ("../bin/" .. outputdir .. "/%{prj.name}")
	objdir ("../bin-int/" .. outputdir .. "/%{prj.name}")
	
	filter "system:windows"
	  systemversion "latest"

	filter "configurations:debug"
      runtime "Debug"
      symbols "On"

	filter "configurations:release"
      runtime "Release"
      optimize "On"
      symbols "On"

	filter "configurations:dist"
      runtime "Release"
      optimize "On"
      symbols "Off"
