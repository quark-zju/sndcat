vcpkg_fail_port_install(ON_TARGET "uwp")
vcpkg_from_github(
    OUT_SOURCE_PATH SOURCE_PATH
    REPO audacity/audacity
    REF Audacity-3.0.0
    SHA512 f75f2f62c9a8e100b8b138826554ca8af43467900b138c4dc6189c1237c961c525e3043928b63ee25831820ae02bac68aa977e6334c32158eec3821771a91701
)

string(COMPARE EQUAL ${VCPKG_LIBRARY_LINKAGE} dynamic PA_BUILD_SHARED)
string(COMPARE EQUAL ${VCPKG_LIBRARY_LINKAGE} static PA_BUILD_STATIC)

# NOTE: the ASIO backend will be built automatically if the ASIO-SDK is provided
# in a sibling folder of the portaudio source in vcpkg/buildtrees/portaudio/src
vcpkg_configure_cmake(
    SOURCE_PATH ${SOURCE_PATH}/lib-src/portaudio-v19
    PREFER_NINJA
    OPTIONS
        -DPA_USE_WASAPI=ON
        -DPA_LIBNAME_ADD_SUFFIX=OFF
        -DPA_ENABLE_DEBUG_OUTPUT=OFF
        -DPA_BUILD_SHARED=${PA_BUILD_SHARED}
        -DPA_BUILD_STATIC=${PA_BUILD_STATIC}
        -DPA_DLL_LINK_WITH_STATIC_RUNTIME=OFF
    OPTIONS_DEBUG
        -DPA_ENABLE_DEBUG_OUTPUT:BOOL=ON
)

vcpkg_install_cmake()
vcpkg_fixup_cmake_targets(CONFIG_PATH lib/cmake/${PORT})
vcpkg_copy_pdbs()

file(REMOVE_RECURSE ${CURRENT_PACKAGES_DIR}/debug/include)
file(REMOVE_RECURSE ${CURRENT_PACKAGES_DIR}/debug/share)

if(VCPKG_LIBRARY_LINKAGE STREQUAL static)
    file(REMOVE_RECURSE ${CURRENT_PACKAGES_DIR}/bin ${CURRENT_PACKAGES_DIR}/debug/bin)
endif()

# Handle copyright
file(INSTALL ${SOURCE_PATH}/LICENSE.txt DESTINATION ${CURRENT_PACKAGES_DIR}/share/portaudio RENAME copyright)