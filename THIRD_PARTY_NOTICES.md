# Third-party notices

GIS Studio is MIT OR Apache-2.0 licensed. It is built on the open-source software listed below, whose authors we thank. Generated 2026-10-22 by `scripts/third_party_notices.py`; licenses are as declared by each package. Full license texts are in each package's source distribution (linked).

## Licensing note for the macOS installer

The bundled GDAL build (from Homebrew) links these libraries, which are licensed under the GPL only. Distributing the installer with them makes that binary distribution subject to the GPL (source must be offered). GIS Studio's own source code remains MIT OR Apache-2.0. A GDAL build without these optional drivers (PDF via poppler, HEIF/HEVC via x265, SpatiaLite topology via librttopo) avoids this.

- **librttopo 1.1.0** — GPL-2.0-or-later — RT Topology Library (librttopo.1.dylib)
- **poppler 26.10.0** — GPL-2.0-only OR GPL-3.0-only — PDF rendering library (based on the xpdf-3.0 code base) (libpoppler.159.0.0.dylib)
- **x265 4.3** — GPL-2.0-or-later — H.265/HEVC encoder (libx265.216.dylib)

## Bundled data

| Data | License / terms | Source |
|---|---|---|
| EPSG Geodetic Parameter Dataset (inside PROJ's proj.db) | EPSG Dataset Terms of Use, © IOGP | https://epsg.org/terms-of-use.html |
| PROJ data files (proj.db, grids) | MIT (PROJ); grids per their providers | https://proj.org/ |
| GDAL data files | MIT | https://gdal.org/ |
| DuckDB spatial extension | MIT | https://github.com/duckdb/duckdb-spatial |
| World Magnetic Model 2025 coefficients (via `geomagnetism`) | Public domain (NOAA NCEI / British Geological Survey) | https://www.ncei.noaa.gov/products/world-magnetic-model |
| Extra CRS definitions (`crates/engine/assets/extra_crs.json`) | Derived from PROJ / EPSG (terms above) | https://proj.org/ |

## Native libraries bundled in the macOS app (187 files from 85 projects)

| Project | Version | License | Home page | Files |
|---|---|---|---|---|
| abseil | 20260817.0 | Apache-2.0 | https://abseil.io | libabsl_base.2601.0.0.dylib, libabsl_borrowed_fixup_buffer.2601.0.0.dylib, libabsl_city.2601.0.0.dylib, libabsl_civil_time.2601.0.0.dylib, libabsl_cord.2601.0.0.dylib, libabsl_cord_internal.2601.0.0.dylib, libabsl_cordz_functions.2601.0.0.dylib, libabsl_cordz_handle.2601.0.0.dylib, libabsl_cordz_info.2601.0.0.dylib, libabsl_crc32c.2601.0.0.dylib, libabsl_crc_cord_state.2601.0.0.dylib, libabsl_crc_cpu_detect.2601.0.0.dylib, libabsl_crc_internal.2601.0.0.dylib, libabsl_debugging_internal.2601.0.0.dylib, libabsl_decode_rust_punycode.2601.0.0.dylib, libabsl_demangle_internal.2601.0.0.dylib, libabsl_demangle_rust.2601.0.0.dylib, libabsl_examine_stack.2601.0.0.dylib, libabsl_exponential_biased.2601.0.0.dylib, libabsl_flags_commandlineflag.2601.0.0.dylib, libabsl_flags_commandlineflag_internal.2601.0.0.dylib, libabsl_flags_config.2601.0.0.dylib, libabsl_flags_internal.2601.0.0.dylib, libabsl_flags_marshalling.2601.0.0.dylib, libabsl_flags_private_handle_accessor.2601.0.0.dylib, libabsl_flags_program_name.2601.0.0.dylib, libabsl_flags_reflection.2601.0.0.dylib, libabsl_graphcycles_internal.2601.0.0.dylib, libabsl_hash.2601.0.0.dylib, libabsl_hashtablez_sampler.2601.0.0.dylib, libabsl_int128.2601.0.0.dylib, libabsl_kernel_timeout_internal.2601.0.0.dylib, libabsl_leak_check.2601.0.0.dylib, libabsl_log_entry.2601.0.0.dylib, libabsl_log_globals.2601.0.0.dylib, libabsl_log_internal_check_op.2601.0.0.dylib, libabsl_log_internal_conditions.2601.0.0.dylib, libabsl_log_internal_fnmatch.2601.0.0.dylib, libabsl_log_internal_format.2601.0.0.dylib, libabsl_log_internal_globals.2601.0.0.dylib, libabsl_log_internal_log_sink_set.2601.0.0.dylib, libabsl_log_internal_message.2601.0.0.dylib, libabsl_log_internal_nullguard.2601.0.0.dylib, libabsl_log_internal_proto.2601.0.0.dylib, libabsl_log_internal_structured_proto.2601.0.0.dylib, libabsl_log_severity.2601.0.0.dylib, libabsl_log_sink.2601.0.0.dylib, libabsl_malloc_internal.2601.0.0.dylib, libabsl_raw_hash_set.2601.0.0.dylib, libabsl_raw_logging_internal.2601.0.0.dylib, libabsl_spinlock_wait.2601.0.0.dylib, libabsl_stacktrace.2601.0.0.dylib, libabsl_str_format_internal.2601.0.0.dylib, libabsl_strerror.2601.0.0.dylib, libabsl_strings.2601.0.0.dylib, libabsl_strings_internal.2601.0.0.dylib, libabsl_symbolize.2601.0.0.dylib, libabsl_synchronization.2601.0.0.dylib, libabsl_throw_delegate.2601.0.0.dylib, libabsl_time.2601.0.0.dylib, libabsl_time_zone.2601.0.0.dylib, libabsl_tracing_internal.2601.0.0.dylib, libabsl_utf8_for_code_point.2601.0.0.dylib, libabsl_vlog_config_internal.2601.0.0.dylib |
| aom | 3.15.2 | BSD-2-Clause | https://aomedia.googlesource.com/aom | libaom.3.13.3.dylib |
| apache-arrow | 26.0.0 | Apache-2.0 | https://arrow.apache.org/ | libarrow.2300.1.0.dylib, libarrow_acero.2300.1.0.dylib, libarrow_compute.2300.1.0.dylib, libarrow_dataset.2300.1.0.dylib, libparquet.2300.1.0.dylib |
| aws-c-auth | 1.0.1 | Apache-2.0 | https://github.com/awslabs/aws-c-auth | libaws-c-auth.1.0.0.dylib |
| aws-c-cal | 1.0.0 | Apache-2.0 | https://github.com/awslabs/aws-c-cal | libaws-c-cal.1.0.0.dylib |
| aws-c-common | 1.0.3 | Apache-2.0 | https://github.com/awslabs/aws-c-common | libaws-c-common.1.0.0.dylib |
| aws-c-compression | 1.0.0 | Apache-2.0 | https://github.com/awslabs/aws-c-compression | libaws-c-compression.1.0.0.dylib |
| aws-c-event-stream | 1.0.0 | Apache-2.0 | https://github.com/awslabs/aws-c-event-stream | libaws-c-event-stream.1.0.0.dylib |
| aws-c-http | 1.1.1 | Apache-2.0 | https://github.com/awslabs/aws-c-http | libaws-c-http.1.0.0.dylib |
| aws-c-io | 1.1.0 | Apache-2.0 | https://github.com/awslabs/aws-c-io | libaws-c-io.1.0.0.dylib |
| aws-c-mqtt | 1.1.2 | Apache-2.0 | https://github.com/awslabs/aws-c-mqtt | libaws-c-mqtt.1.0.0.dylib |
| aws-c-s3 | 1.3.1 | Apache-2.0 | https://github.com/awslabs/aws-c-s3 | libaws-c-s3.1.0.0.dylib |
| aws-c-sdkutils | 1.0.0 | Apache-2.0 | https://github.com/awslabs/aws-c-sdkutils | libaws-c-sdkutils.1.0.0.dylib |
| aws-checksums | 1.0.1 | Apache-2.0 | https://github.com/awslabs/aws-checksums | libaws-checksums.1.0.0.dylib |
| aws-crt-cpp | 0.44.2 | Apache-2.0 | https://github.com/awslabs/aws-crt-cpp | libaws-crt-cpp.dylib |
| aws-sdk-cpp | 1.11.915 | Apache-2.0 | https://github.com/aws/aws-sdk-cpp | libaws-cpp-sdk-cognito-identity.dylib, libaws-cpp-sdk-core.dylib, libaws-cpp-sdk-identity-management.dylib, libaws-cpp-sdk-s3.dylib, libaws-cpp-sdk-sts.dylib |
| boost | 1.92.0 | BSL-1.0 | https://www.boost.org/ | libboost_atomic.dylib, libboost_chrono.dylib, libboost_container.dylib, libboost_date_time.dylib, libboost_serialization.dylib, libboost_thread.dylib |
| brotli | 1.2.1 | MIT | https://github.com/google/brotli | libbrotlicommon.1.2.0.dylib, libbrotlidec.1.2.0.dylib, libbrotlienc.1.2.0.dylib |
| c-blosc | 1.21.6 | BSD-3-Clause | https://www.blosc.org/ | libblosc.1.21.6.dylib |
| cfitsio | 4.7.0 | CFITSIO | https://heasarc.gsfc.nasa.gov/docs/software/fitsio/fitsio.html | libcfitsio.4.6.3.dylib |
| duckdb | 2.0.0 | MIT | https://www.duckdb.org | libduckdb.dylib |
| fontconfig | 2.18.3 | HPND-sell-variant AND Unicode-3.0 AND MIT-Modern-Variant AND MIT AND LicenseRef-Homebrew-public-domain | https://wiki.freedesktop.org/www/Software/fontconfig/ | libfontconfig.1.dylib |
| freetype | 2.15.0 | FTL | https://www.freetype.org/ | libfreetype.6.dylib |
| freexl | 2.0.0 | MPL-1.1 OR GPL-2.0-or-later OR LGPL-2.1-or-later | https://www.gaia-gis.it/fossil/freexl/index | libfreexl.1.dylib |
| gdal | 3.13.3 | MIT | https://gdal.org/en/stable/ | libgdal.38.3.12.3.dylib |
| geos | 3.15.0 | LGPL-2.1-or-later | https://libgeos.org/ | libgeos.3.14.1.dylib, libgeos_c.1.20.5.dylib |
| gettext | 1.0 | LGPL-2.1-or-later (libintl; the gettext tools are GPL-3.0-or-later) | https://www.gnu.org/software/gettext/ | libintl.8.dylib |
| giflib | 6.1.3 | MIT | https://giflib.sourceforge.net/ | libgif.7.2.0.dylib |
| gmp | 6.3.0 | LGPL-3.0-or-later OR GPL-2.0-or-later | https://gmplib.org/ | libgmp.10.dylib, libgmpxx.4.dylib |
| gpgme | 2.2.0 | LGPL-2.1-or-later | https://www.gnupg.org/related_software/gpgme/ | libgpgme.45.dylib |
| gpgmepp | 2.2.0 | LGPL-2.1-or-later | https://www.gnupg.org/related_software/gpgme/ | libgpgmepp.7.0.0.dylib |
| hdf5 | 2.2.0 | BSD-3-Clause | https://www.hdfgroup.org/solutions/hdf5/ | libhdf5.320.1.1.dylib, libhdf5_hl.320.0.2.dylib |
| highway | 1.4.0 | Apache-2.0 OR BSD-3-Clause | https://github.com/google/highway | libhwy.1.3.0.dylib |
| imath | 3.2.3 | BSD-3-Clause | https://imath.readthedocs.io/en/latest/ | libImath-3_2.30.3.2.2.dylib |
| jpeg-turbo | 3.2.0 | IJG AND Zlib AND BSD-3-Clause | https://www.libjpeg-turbo.org/ | libjpeg.8.3.2.dylib |
| jpeg-xl | 0.12.0 | BSD-3-Clause | https://jpeg.org/jpegxl/index.html | libjxl.0.11.2.dylib, libjxl_cms.0.11.2.dylib, libjxl_threads.0.11.2.dylib |
| json-c | 0.19 | MIT | https://github.com/json-c/json-c/wiki | libjson-c.5.4.0.dylib |
| krb5 | 1.22.2 | BSD-2-Clause AND BSD-2-Clause-first-lines AND BSD-3-Clause AND BSD-4-Clause AND Brian-Gladman-2-Clause AND CMU-Mach-nodoc AND FSFULLRWD AND HPND AND HPND-export2-US AND HPND-export-US AND HPND-export-US-acknowledgement AND HPND-export-US-modify AND ISC AND MIT AND MIT-CMU AND OLDAP-2.8 AND OpenVision AND (BSD-2-Clause OR GPL-2.0-or-later) | https://web.mit.edu/kerberos/ | libcom_err.3.0.dylib, libgssapi_krb5.2.2.dylib, libk5crypto.3.1.dylib, libkrb5.3.3.dylib, libkrb5support.1.1.dylib |
| libaec | 1.1.7 | BSD-2-Clause | https://github.com/Deutsches-Klimarechenzentrum/libaec | libaec.0.1.6.dylib, libsz.2.0.1.dylib |
| libarchive | 3.8.10 | BSD-2-Clause | https://www.libarchive.org | libarchive.13.dylib |
| libassuan | 3.0.2 | LGPL-2.1-or-later AND GPL-3.0-or-later AND FSFULLR | https://www.gnupg.org/related_software/libassuan/ | libassuan.9.dylib |
| libb2 | 0.98.1 | CC0-1.0 | https://blake2.net/ | libb2.1.dylib |
| libde265 | 1.1.3 | LGPL-3.0-or-later | https://github.com/strukturag/libde265 | libde265.0.1.11.dylib |
| libdeflate | 1.26 | MIT | https://github.com/ebiggers/libdeflate | libdeflate.0.dylib |
| libgeotiff | 1.7.4 | MIT | https://github.com/OSGeo/libgeotiff | libgeotiff.5.dylib |
| libgpg-error | 1.61 | LGPL-2.1-or-later | https://www.gnupg.org/related_software/libgpg-error/ | libgpg-error.0.dylib |
| libheif | 1.23.6 | LGPL-3.0-or-later | https://www.libde265.org/ | libheif.1.21.2.dylib |
| libkml | 1.3.0 | BSD-3-Clause | https://github.com/libkml/libkml | libkmlbase.1.3.0.dylib, libkmldom.1.3.0.dylib, libkmlengine.1.3.0.dylib |
| liblerc | 4.2.0 | Apache-2.0 | https://github.com/Esri/lerc | libLerc.4.dylib |
| libpng | 1.6.59 | libpng-2.0 | https://www.libpng.org/pub/png/libpng.html | libpng16.16.dylib |
| libpq | 18.6 | PostgreSQL | https://www.postgresql.org/docs/current/libpq.html | libpq.5.dylib |
| librttopo | 1.1.0 | GPL-2.0-or-later | https://git.osgeo.org/gitea/rttopo/librttopo | librttopo.1.dylib |
| libspatialite | 5.1.0 | MPL-1.1 OR GPL-2.0-or-later OR LGPL-2.1-or-later | https://www.gaia-gis.it/fossil/libspatialite/index | libspatialite.8.dylib |
| libtiff | 4.7.2 | libtiff | https://libtiff.gitlab.io/libtiff/ | libtiff.6.dylib |
| libtool | 2.6.2 | LGPL-2.1-or-later (libltdl; the libtool scripts are GPL-2.0-or-later) | https://www.gnu.org/software/libtool/ | libltdl.7.dylib |
| libvmaf | 3.2.1 | BSD-2-Clause-Patent | https://github.com/Netflix/vmaf | libvmaf.3.dylib |
| libxml2 | 2.15.4 | MIT | http://xmlsoft.org/ | libxml2.16.dylib |
| little-cms2 | 2.19.1 | MIT | https://www.littlecms.com/ | liblcms2.2.dylib |
| lz4 | 1.10.0 | BSD-2-Clause | https://lz4.github.io/lz4/ | liblz4.1.10.0.dylib |
| minizip | 1.3.2 | Zlib | https://www.winimage.com/zLibDll/minizip.html | libminizip.1.dylib |
| mpfr | 4.2.2 | LGPL-3.0-or-later | https://www.mpfr.org/ | libmpfr.6.dylib |
| netcdf | 4.10.1 | BSD-3-Clause | https://www.unidata.ucar.edu/software/netcdf/ | libnetcdf.22.dylib |
| nspr | 4.40 | MPL-2.0 | https://hg.mozilla.org/projects/nspr | libnspr4.dylib, libplc4.dylib, libplds4.dylib |
| nss | 3.132 | MPL-2.0 | https://firefox-source-docs.mozilla.org/security/nss/index.html | libnss3.dylib, libnssutil3.dylib, libsmime3.dylib, libssl3.dylib |
| openexr | 3.5.2 | BSD-3-Clause | https://www.openexr.com/ | libIex-3_4.33.3.4.10.dylib, libIlmThread-3_4.33.3.4.10.dylib, libOpenEXR-3_4.33.3.4.10.dylib, libOpenEXRCore-3_4.33.3.4.10.dylib, libOpenEXRUtil-3_4.33.3.4.10.dylib |
| openjpeg | 2.5.4 | BSD-2-Clause | https://www.openjpeg.org/ | libopenjp2.2.5.4.dylib |
| openjph | 0.32.0 | BSD-2-Clause | https://github.com/aous72/OpenJPH | libopenjph.0.27.0.dylib |
| openssl@3 | 3.5.9 | Apache-2.0 | https://openssl-library.org | libcrypto.3.dylib, libssl.3.dylib |
| pcre2 | 10.49 | BSD-3-Clause | https://www.pcre.org/ | libpcre2-8.0.dylib |
| poppler | 26.10.0 | GPL-2.0-only OR GPL-3.0-only | https://poppler.freedesktop.org/ | libpoppler.159.0.0.dylib |
| proj | 9.9.0 | MIT | https://proj.org/ | libproj.25.9.8.1.dylib |
| qhull | 2020.2 | Qhull | http://www.qhull.org/ | libqhull_r.8.0.2.dylib |
| re2 | 2025-11-05 | BSD-3-Clause | https://github.com/google/re2 | libre2.11.0.0.dylib |
| sfcgal | 2.3.0 | LGPL-2.0-or-later | https://sfcgal.gitlab.io/SFCGAL/ | libSFCGAL.2.2.0.dylib |
| snappy | 1.3.1 | BSD-3-Clause | https://google.github.io/snappy/ | libsnappy.1.2.2.dylib |
| sqlite | 3.54.1 | blessing | https://sqlite.org/index.html | libsqlite3.3.53.1.dylib |
| thrift | 0.25.0 | Apache-2.0 | https://thrift.apache.org/ | libthrift-0.22.0.dylib |
| unixodbc | 2.3.14 | LGPL-2.1-or-later (libodbc) | https://www.unixodbc.org/ | libodbc.2.dylib, libodbcinst.2.dylib |
| uriparser | 1.0.2 | BSD-3-Clause | https://uriparser.github.io/ | liburiparser.1.2.0.dylib |
| utf8proc | 2.12.0 | MIT AND Unicode-DFS-2015 | https://juliastrings.github.io/utf8proc/ | libutf8proc.3.2.3.dylib |
| webp | 1.6.0 | BSD-3-Clause | https://developers.google.com/speed/webp/ | libsharpyuv.0.1.2.dylib, libwebp.7.2.0.dylib |
| x265 | 4.3 | GPL-2.0-or-later | https://github.com/Multicorewareinc/x265 | libx265.216.dylib |
| xerces-c | 3.3.0 | Apache-2.0 | https://xerces.apache.org/xerces-c/ | libxerces-c-3.3.dylib |
| xz | 5.8.4 | 0BSD (liblzma; some xz tools are GPL-2.0-or-later) | https://tukaani.org/xz/ | liblzma.5.dylib |
| zstd | 1.5.7 | (BSD-3-Clause OR GPL-2.0-only) AND BSD-2-Clause AND MIT | https://facebook.github.io/zstd/ | libzstd.1.5.7.dylib |

## JavaScript packages bundled in the user interface (150)

| Package | Version | License |
|---|---|---|
| [@babel/runtime](https://www.npmjs.com/package/@babel/runtime) | 7.29.10 | MIT |
| [@emotion/hash](https://www.npmjs.com/package/@emotion/hash) | 0.9.2 | MIT |
| [@floating-ui/core](https://www.npmjs.com/package/@floating-ui/core) | 1.8.0 | MIT |
| [@floating-ui/devtools](https://www.npmjs.com/package/@floating-ui/devtools) | 0.2.3 | MIT |
| [@floating-ui/dom](https://www.npmjs.com/package/@floating-ui/dom) | 1.8.0 | MIT |
| [@floating-ui/utils](https://www.npmjs.com/package/@floating-ui/utils) | 0.2.12 | MIT |
| [@fluentui/keyboard-keys](https://www.npmjs.com/package/@fluentui/keyboard-keys) | 9.0.9 | MIT |
| [@fluentui/priority-overflow](https://www.npmjs.com/package/@fluentui/priority-overflow) | 9.4.3 | MIT |
| [@fluentui/react-accordion](https://www.npmjs.com/package/@fluentui/react-accordion) | 9.12.5 | MIT |
| [@fluentui/react-alert](https://www.npmjs.com/package/@fluentui/react-alert) | 9.0.2-beta.1 | MIT |
| [@fluentui/react-aria](https://www.npmjs.com/package/@fluentui/react-aria) | 9.17.16 | MIT |
| [@fluentui/react-avatar](https://www.npmjs.com/package/@fluentui/react-avatar) | 9.11.9 | MIT |
| [@fluentui/react-badge](https://www.npmjs.com/package/@fluentui/react-badge) | 9.5.7 | MIT |
| [@fluentui/react-breadcrumb](https://www.npmjs.com/package/@fluentui/react-breadcrumb) | 9.4.7 | MIT |
| [@fluentui/react-button](https://www.npmjs.com/package/@fluentui/react-button) | 9.11.2 | MIT |
| [@fluentui/react-card](https://www.npmjs.com/package/@fluentui/react-card) | 9.7.4 | MIT |
| [@fluentui/react-carousel](https://www.npmjs.com/package/@fluentui/react-carousel) | 9.9.15 | MIT |
| [@fluentui/react-checkbox](https://www.npmjs.com/package/@fluentui/react-checkbox) | 9.6.6 | MIT |
| [@fluentui/react-color-picker](https://www.npmjs.com/package/@fluentui/react-color-picker) | 9.3.2 | MIT |
| [@fluentui/react-combobox](https://www.npmjs.com/package/@fluentui/react-combobox) | 9.17.8 | MIT |
| [@fluentui/react-components](https://www.npmjs.com/package/@fluentui/react-components) | 9.74.10 | MIT |
| [@fluentui/react-context-selector](https://www.npmjs.com/package/@fluentui/react-context-selector) | 9.2.21 | MIT |
| [@fluentui/react-dialog](https://www.npmjs.com/package/@fluentui/react-dialog) | 9.18.6 | MIT |
| [@fluentui/react-divider](https://www.npmjs.com/package/@fluentui/react-divider) | 9.7.6 | MIT |
| [@fluentui/react-drawer](https://www.npmjs.com/package/@fluentui/react-drawer) | 9.13.5 | MIT |
| [@fluentui/react-field](https://www.npmjs.com/package/@fluentui/react-field) | 9.5.6 | MIT |
| [@fluentui/react-icons](https://www.npmjs.com/package/@fluentui/react-icons) | 2.0.344 | MIT |
| [@fluentui/react-image](https://www.npmjs.com/package/@fluentui/react-image) | 9.4.6 | MIT |
| [@fluentui/react-infobutton](https://www.npmjs.com/package/@fluentui/react-infobutton) | 9.0.1-beta.1 | MIT |
| [@fluentui/react-infolabel](https://www.npmjs.com/package/@fluentui/react-infolabel) | 9.4.28 | MIT |
| [@fluentui/react-input](https://www.npmjs.com/package/@fluentui/react-input) | 9.8.8 | MIT |
| [@fluentui/react-jsx-runtime](https://www.npmjs.com/package/@fluentui/react-jsx-runtime) | 9.4.7 | MIT |
| [@fluentui/react-label](https://www.npmjs.com/package/@fluentui/react-label) | 9.4.6 | MIT |
| [@fluentui/react-link](https://www.npmjs.com/package/@fluentui/react-link) | 9.8.6 | MIT |
| [@fluentui/react-list](https://www.npmjs.com/package/@fluentui/react-list) | 9.7.0 | MIT |
| [@fluentui/react-menu](https://www.npmjs.com/package/@fluentui/react-menu) | 9.25.7 | MIT |
| [@fluentui/react-message-bar](https://www.npmjs.com/package/@fluentui/react-message-bar) | 9.7.8 | MIT |
| [@fluentui/react-motion](https://www.npmjs.com/package/@fluentui/react-motion) | 9.16.5 | MIT |
| [@fluentui/react-motion-components-preview](https://www.npmjs.com/package/@fluentui/react-motion-components-preview) | 0.15.10 | MIT |
| [@fluentui/react-nav](https://www.npmjs.com/package/@fluentui/react-nav) | 9.4.8 | MIT |
| [@fluentui/react-overflow](https://www.npmjs.com/package/@fluentui/react-overflow) | 9.9.5 | MIT |
| [@fluentui/react-persona](https://www.npmjs.com/package/@fluentui/react-persona) | 9.7.11 | MIT |
| [@fluentui/react-popover](https://www.npmjs.com/package/@fluentui/react-popover) | 9.14.10 | MIT |
| [@fluentui/react-portal](https://www.npmjs.com/package/@fluentui/react-portal) | 9.8.17 | MIT |
| [@fluentui/react-positioning](https://www.npmjs.com/package/@fluentui/react-positioning) | 9.23.4 | MIT |
| [@fluentui/react-progress](https://www.npmjs.com/package/@fluentui/react-progress) | 9.5.7 | MIT |
| [@fluentui/react-provider](https://www.npmjs.com/package/@fluentui/react-provider) | 9.22.22 | MIT |
| [@fluentui/react-radio](https://www.npmjs.com/package/@fluentui/react-radio) | 9.6.7 | MIT |
| [@fluentui/react-rating](https://www.npmjs.com/package/@fluentui/react-rating) | 9.4.7 | MIT |
| [@fluentui/react-search](https://www.npmjs.com/package/@fluentui/react-search) | 9.4.8 | MIT |
| [@fluentui/react-select](https://www.npmjs.com/package/@fluentui/react-select) | 9.5.7 | MIT |
| [@fluentui/react-shared-contexts](https://www.npmjs.com/package/@fluentui/react-shared-contexts) | 9.26.4 | MIT |
| [@fluentui/react-skeleton](https://www.npmjs.com/package/@fluentui/react-skeleton) | 9.7.7 | MIT |
| [@fluentui/react-slider](https://www.npmjs.com/package/@fluentui/react-slider) | 9.6.7 | MIT |
| [@fluentui/react-spinbutton](https://www.npmjs.com/package/@fluentui/react-spinbutton) | 9.6.8 | MIT |
| [@fluentui/react-spinner](https://www.npmjs.com/package/@fluentui/react-spinner) | 9.8.7 | MIT |
| [@fluentui/react-swatch-picker](https://www.npmjs.com/package/@fluentui/react-swatch-picker) | 9.6.3 | MIT |
| [@fluentui/react-switch](https://www.npmjs.com/package/@fluentui/react-switch) | 9.7.7 | MIT |
| [@fluentui/react-table](https://www.npmjs.com/package/@fluentui/react-table) | 9.19.23 | MIT |
| [@fluentui/react-tabs](https://www.npmjs.com/package/@fluentui/react-tabs) | 9.12.6 | MIT |
| [@fluentui/react-tabster](https://www.npmjs.com/package/@fluentui/react-tabster) | 9.26.19 | MIT |
| [@fluentui/react-tag-picker](https://www.npmjs.com/package/@fluentui/react-tag-picker) | 9.10.6 | MIT |
| [@fluentui/react-tags](https://www.npmjs.com/package/@fluentui/react-tags) | 9.9.8 | MIT |
| [@fluentui/react-teaching-popover](https://www.npmjs.com/package/@fluentui/react-teaching-popover) | 9.7.8 | MIT |
| [@fluentui/react-text](https://www.npmjs.com/package/@fluentui/react-text) | 9.6.21 | MIT |
| [@fluentui/react-textarea](https://www.npmjs.com/package/@fluentui/react-textarea) | 9.7.8 | MIT |
| [@fluentui/react-theme](https://www.npmjs.com/package/@fluentui/react-theme) | 9.2.2 | MIT |
| [@fluentui/react-toast](https://www.npmjs.com/package/@fluentui/react-toast) | 9.8.5 | MIT |
| [@fluentui/react-toolbar](https://www.npmjs.com/package/@fluentui/react-toolbar) | 9.8.6 | MIT |
| [@fluentui/react-tooltip](https://www.npmjs.com/package/@fluentui/react-tooltip) | 9.10.8 | MIT |
| [@fluentui/react-tree](https://www.npmjs.com/package/@fluentui/react-tree) | 9.16.9 | MIT |
| [@fluentui/react-utilities](https://www.npmjs.com/package/@fluentui/react-utilities) | 9.27.0 | MIT |
| [@fluentui/react-virtualizer](https://www.npmjs.com/package/@fluentui/react-virtualizer) | 9.0.0-alpha.116 | MIT |
| [@fluentui/tokens](https://www.npmjs.com/package/@fluentui/tokens) | 1.0.0-alpha.24 | MIT |
| [@griffel/core](https://www.npmjs.com/package/@griffel/core) | 1.21.4 | MIT |
| [@griffel/react](https://www.npmjs.com/package/@griffel/react) | 1.7.8 | MIT |
| [@griffel/style-types](https://www.npmjs.com/package/@griffel/style-types) | 1.4.3 | MIT |
| [@mapbox/jsonlint-lines-primitives](https://www.npmjs.com/package/@mapbox/jsonlint-lines-primitives) | 2.0.3 | MIT |
| [@mapbox/point-geometry](https://www.npmjs.com/package/@mapbox/point-geometry) | 1.1.0 | ISC |
| [@mapbox/tiny-sdf](https://www.npmjs.com/package/@mapbox/tiny-sdf) | 2.3.0 | BSD-2-Clause |
| [@mapbox/unitbezier](https://www.npmjs.com/package/@mapbox/unitbezier) | 1.0.0 | BSD-2-Clause |
| [@mapbox/vector-tile](https://www.npmjs.com/package/@mapbox/vector-tile) | 3.0.0 | BSD-3-Clause |
| [@maplibre/geojson-vt](https://www.npmjs.com/package/@maplibre/geojson-vt) | 6.3.0 | ISC |
| [@maplibre/maplibre-gl-style-spec](https://www.npmjs.com/package/@maplibre/maplibre-gl-style-spec) | 26.4.4 | ISC |
| [@maplibre/mlt](https://www.npmjs.com/package/@maplibre/mlt) | 1.3.0 | (MIT OR Apache-2.0) |
| [@maplibre/vt-pbf](https://www.npmjs.com/package/@maplibre/vt-pbf) | 4.3.2 | MIT |
| [@swc/helpers](https://www.npmjs.com/package/@swc/helpers) | 0.5.23 | Apache-2.0 |
| [@tanstack/react-virtual](https://www.npmjs.com/package/@tanstack/react-virtual) | 3.14.21 | MIT |
| [@tanstack/virtual-core](https://www.npmjs.com/package/@tanstack/virtual-core) | 3.19.6 | MIT |
| [@tauri-apps/api](https://www.npmjs.com/package/@tauri-apps/api) | 2.12.4 | Apache-2.0 OR MIT |
| [@tauri-apps/plugin-dialog](https://www.npmjs.com/package/@tauri-apps/plugin-dialog) | 2.8.3 | MIT OR Apache-2.0 |
| [@types/d3-color](https://www.npmjs.com/package/@types/d3-color) | 3.1.3 | MIT |
| [@types/d3-drag](https://www.npmjs.com/package/@types/d3-drag) | 3.0.7 | MIT |
| [@types/d3-interpolate](https://www.npmjs.com/package/@types/d3-interpolate) | 3.0.4 | MIT |
| [@types/d3-selection](https://www.npmjs.com/package/@types/d3-selection) | 3.0.12 | MIT |
| [@types/d3-transition](https://www.npmjs.com/package/@types/d3-transition) | 3.0.9 | MIT |
| [@types/d3-zoom](https://www.npmjs.com/package/@types/d3-zoom) | 3.0.9 | MIT |
| [@types/geojson](https://www.npmjs.com/package/@types/geojson) | 7946.0.16 | MIT |
| [@xyflow/react](https://www.npmjs.com/package/@xyflow/react) | 12.12.0 | MIT |
| [@xyflow/system](https://www.npmjs.com/package/@xyflow/system) | 0.0.83 | MIT |
| [bidi-js](https://www.npmjs.com/package/bidi-js) | 1.1.0 | MIT |
| [classcat](https://www.npmjs.com/package/classcat) | 5.0.5 | MIT |
| [csstype](https://www.npmjs.com/package/csstype) | 3.2.3 | MIT |
| [d3-color](https://www.npmjs.com/package/d3-color) | 3.1.0 | ISC |
| [d3-dispatch](https://www.npmjs.com/package/d3-dispatch) | 3.0.1 | ISC |
| [d3-drag](https://www.npmjs.com/package/d3-drag) | 3.0.0 | ISC |
| [d3-ease](https://www.npmjs.com/package/d3-ease) | 3.0.1 | BSD-3-Clause |
| [d3-interpolate](https://www.npmjs.com/package/d3-interpolate) | 3.0.1 | ISC |
| [d3-selection](https://www.npmjs.com/package/d3-selection) | 3.0.0 | ISC |
| [d3-timer](https://www.npmjs.com/package/d3-timer) | 3.0.1 | ISC |
| [d3-transition](https://www.npmjs.com/package/d3-transition) | 3.0.1 | ISC |
| [d3-zoom](https://www.npmjs.com/package/d3-zoom) | 3.0.0 | ISC |
| [dockview](https://www.npmjs.com/package/dockview) | 8.6.1 | MIT |
| [dockview-core](https://www.npmjs.com/package/dockview-core) | 8.6.1 | MIT |
| [dockview-react](https://www.npmjs.com/package/dockview-react) | 8.6.1 | MIT |
| [earcut](https://www.npmjs.com/package/earcut) | 3.2.4 | ISC |
| [embla-carousel](https://www.npmjs.com/package/embla-carousel) | 8.6.0 | MIT |
| [embla-carousel-autoplay](https://www.npmjs.com/package/embla-carousel-autoplay) | 8.6.0 | MIT |
| [embla-carousel-fade](https://www.npmjs.com/package/embla-carousel-fade) | 8.6.0 | MIT |
| [geographiclib-geodesic](https://www.npmjs.com/package/geographiclib-geodesic) | 2.2.0 | MIT |
| [geomagnetism](https://www.npmjs.com/package/geomagnetism) | 0.2.0 | Apache-2.0 |
| [gl-matrix](https://www.npmjs.com/package/gl-matrix) | 3.4.4 | MIT |
| [json-stringify-pretty-compact](https://www.npmjs.com/package/json-stringify-pretty-compact) | 4.0.0 | MIT |
| [kdbush](https://www.npmjs.com/package/kdbush) | 4.1.0 | ISC |
| [keyborg](https://www.npmjs.com/package/keyborg) | 3.498.0 | MIT |
| [maplibre-gl](https://www.npmjs.com/package/maplibre-gl) | 6.15.0 | BSD-3-Clause |
| [mgrs](https://www.npmjs.com/package/mgrs) | 1.0.0 | MIT |
| [mgrs](https://www.npmjs.com/package/mgrs) | 2.2.0 | MIT |
| [minimist](https://www.npmjs.com/package/minimist) | 1.2.8 | MIT |
| [murmurhash-js](https://www.npmjs.com/package/murmurhash-js) | 1.0.0 | MIT |
| [pbf](https://www.npmjs.com/package/pbf) | 5.1.2 | BSD-3-Clause |
| [potpack](https://www.npmjs.com/package/potpack) | 2.1.0 | ISC |
| [proj4](https://www.npmjs.com/package/proj4) | 2.22.0 | MIT |
| [protocol-buffers-schema](https://www.npmjs.com/package/protocol-buffers-schema) | 3.6.1 | MIT |
| [quickselect](https://www.npmjs.com/package/quickselect) | 3.0.0 | ISC |
| [react](https://www.npmjs.com/package/react) | 19.3.1 | MIT |
| [react-dom](https://www.npmjs.com/package/react-dom) | 19.3.1 | MIT |
| [require-from-string](https://www.npmjs.com/package/require-from-string) | 2.0.2 | MIT |
| [resolve-protobuf-schema](https://www.npmjs.com/package/resolve-protobuf-schema) | 2.1.0 | MIT |
| [rtl-css-js](https://www.npmjs.com/package/rtl-css-js) | 1.16.1 | MIT |
| [scheduler](https://www.npmjs.com/package/scheduler) | 0.28.0 | MIT |
| [stylis](https://www.npmjs.com/package/stylis) | 4.4.0 | MIT |
| [tabster](https://www.npmjs.com/package/tabster) | 8.8.1 | MIT |
| [tinyqueue](https://www.npmjs.com/package/tinyqueue) | 3.0.0 | ISC |
| [tslib](https://www.npmjs.com/package/tslib) | 2.8.1 | 0BSD |
| [use-sync-external-store](https://www.npmjs.com/package/use-sync-external-store) | 1.7.0 | MIT |
| [wkt-parser](https://www.npmjs.com/package/wkt-parser) | 1.6.0 | MIT |
| [zundo](https://www.npmjs.com/package/zundo) | 2.3.0 | MIT |
| [zustand](https://www.npmjs.com/package/zustand) | 4.5.7 | MIT |
| [zustand](https://www.npmjs.com/package/zustand) | 5.0.15 | MIT |

## Rust crates compiled into the app (516)

| Crate | Version | License | Repository |
|---|---|---|---|
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 | https://github.com/oyvindln/adler2 |
| ahash | 0.8.12 | MIT OR Apache-2.0 | https://github.com/tkaitchuck/ahash |
| aho-corasick | 1.1.5 | Unlicense OR MIT | https://github.com/BurntSushi/aho-corasick |
| alloc-no-stdlib | 3.0.0 | BSD-3-Clause | https://github.com/dropbox/rust-alloc-no-stdlib |
| alloc-stdlib | 0.3.0 | BSD-3-Clause | https://github.com/dropbox/rust-alloc-no-stdlib |
| allocator-api2 | 0.2.21 | MIT OR Apache-2.0 | https://github.com/zakarumych/allocator-api2 |
| android_system_properties | 0.1.6 | MIT OR Apache-2.0 | https://github.com/nical/android_system_properties |
| anyhow | 1.0.104 | MIT OR Apache-2.0 | https://github.com/dtolnay/anyhow |
| approx | 0.5.1 | Apache-2.0 | https://github.com/brendanzab/approx |
| arrow | 58.4.0 | Apache-2.0 | https://github.com/apache/arrow-rs |
| arrow-arith | 58.4.0 | Apache-2.0 | https://github.com/apache/arrow-rs |
| arrow-array | 58.4.0 | Apache-2.0 AND MIT | https://github.com/apache/arrow-rs |
| arrow-buffer | 58.4.0 | Apache-2.0 | https://github.com/apache/arrow-rs |
| arrow-cast | 58.4.0 | Apache-2.0 | https://github.com/apache/arrow-rs |
| arrow-data | 58.4.0 | Apache-2.0 | https://github.com/apache/arrow-rs |
| arrow-ord | 58.4.0 | Apache-2.0 | https://github.com/apache/arrow-rs |
| arrow-row | 58.4.0 | Apache-2.0 | https://github.com/apache/arrow-rs |
| arrow-schema | 58.4.0 | Apache-2.0 | https://github.com/apache/arrow-rs |
| arrow-select | 58.4.0 | Apache-2.0 | https://github.com/apache/arrow-rs |
| arrow-string | 58.4.0 | Apache-2.0 | https://github.com/apache/arrow-rs |
| ascii | 1.1.0 | Apache-2.0 OR MIT | https://github.com/tomprogrammer/rust-ascii |
| atk | 0.18.2 | MIT | https://github.com/gtk-rs/gtk3-rs |
| atk-sys | 0.18.2 | MIT | https://github.com/gtk-rs/gtk3-rs |
| atoi | 2.0.0 | MIT | https://github.com/pacman82/atoi-rs |
| atomic-waker | 1.1.2 | Apache-2.0 OR MIT | https://github.com/smol-rs/atomic-waker |
| autocfg | 1.5.1 | Apache-2.0 OR MIT | https://github.com/cuviper/autocfg |
| base64 | 0.21.7 | MIT OR Apache-2.0 | https://github.com/marshallpierce/rust-base64 |
| base64 | 0.22.1 | MIT OR Apache-2.0 | https://github.com/marshallpierce/rust-base64 |
| base64 | 0.23.1 | MIT OR Apache-2.0 | https://github.com/marshallpierce/rust-base64 |
| bit-set | 0.8.0 | Apache-2.0 OR MIT | https://github.com/contain-rs/bit-set |
| bit-vec | 0.8.0 | Apache-2.0 OR MIT | https://github.com/contain-rs/bit-vec |
| bitflags | 1.3.2 | MIT/Apache-2.0 | https://github.com/bitflags/bitflags |
| bitflags | 2.13.2 | MIT OR Apache-2.0 | https://github.com/bitflags/bitflags |
| block-buffer | 0.10.4 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| block2 | 0.6.2 | MIT | https://github.com/madsmtm/objc2 |
| brotli | 9.0.0 | BSD-3-Clause AND MIT | https://github.com/dropbox/rust-brotli |
| brotli-decompressor | 6.0.1 | BSD-3-Clause/MIT | https://github.com/dropbox/rust-brotli-decompressor |
| bs58 | 0.5.1 | MIT/Apache-2.0 | https://github.com/Nullus157/bs58-rs |
| bumpalo | 3.20.3 | MIT OR Apache-2.0 | https://github.com/fitzgen/bumpalo |
| bytemuck | 1.25.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/Lokathor/bytemuck |
| byteorder | 1.5.0 | Unlicense OR MIT | https://github.com/BurntSushi/byteorder |
| bytes | 1.12.1 | MIT | https://github.com/tokio-rs/bytes |
| cairo-rs | 0.18.5 | MIT | https://github.com/gtk-rs/gtk-rs-core |
| cairo-sys-rs | 0.18.2 | MIT | https://github.com/gtk-rs/gtk-rs-core |
| camino | 1.2.6 | MIT OR Apache-2.0 | https://github.com/camino-rs/camino |
| cargo-platform | 0.1.9 | MIT OR Apache-2.0 | https://github.com/rust-lang/cargo |
| cargo_metadata | 0.19.2 | MIT | https://github.com/oli-obk/cargo_metadata |
| cargo_toml | 1.0.1 | Apache-2.0 OR MIT | https://gitlab.com/lib.rs/cargo_toml |
| cast | 0.3.0 | MIT OR Apache-2.0 | https://github.com/japaric/cast.rs |
| cc | 1.9.0 | MIT OR Apache-2.0 | https://github.com/rust-lang/cc-rs |
| cesu8 | 1.1.0 | Apache-2.0/MIT | https://github.com/emk/cesu8-rs |
| cfb | 0.14.0 | MIT | https://github.com/mdsteele/rust-cfb |
| cfg-expr | 0.15.8 | MIT OR Apache-2.0 | https://github.com/EmbarkStudios/cfg-expr |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 | https://github.com/rust-lang/cfg-if |
| cfg_aliases | 0.2.2 | MIT | https://github.com/katharostech/cfg_aliases |
| chrono | 0.4.45 | MIT OR Apache-2.0 | https://github.com/chronotope/chrono |
| chunked_transfer | 1.5.0 | MIT OR Apache-2.0 | https://github.com/frewsxcv/rust-chunked-transfer |
| combine | 4.6.9 | MIT | https://github.com/Marwes/combine |
| comfy-table | 7.1.4 | MIT | https://github.com/nukesor/comfy-table |
| const-random | 0.1.18 | MIT OR Apache-2.0 | https://github.com/tkaitchuck/constrandom |
| const-random-macro | 0.1.16 | MIT OR Apache-2.0 | https://github.com/tkaitchuck/constrandom |
| cookie | 0.18.2 | MIT OR Apache-2.0 | https://github.com/SergioBenitez/cookie-rs |
| core-foundation | 0.10.1 | MIT OR Apache-2.0 | https://github.com/servo/core-foundation-rs |
| core-foundation-sys | 0.8.7 | MIT OR Apache-2.0 | https://github.com/servo/core-foundation-rs |
| core-graphics | 0.25.0 | MIT OR Apache-2.0 | https://github.com/servo/core-foundation-rs |
| core-graphics-types | 0.2.0 | MIT OR Apache-2.0 | https://github.com/servo/core-foundation-rs |
| cpufeatures | 0.2.17 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| crc32fast | 1.5.3 | MIT OR Apache-2.0 | https://github.com/srijs/rust-crc32fast |
| crossbeam-channel | 0.5.17 | MIT OR Apache-2.0 | https://github.com/crossbeam-rs/crossbeam |
| crossbeam-utils | 0.8.23 | MIT OR Apache-2.0 | https://github.com/crossbeam-rs/crossbeam |
| crossterm | 0.28.1 | MIT | https://github.com/crossterm-rs/crossterm |
| crossterm_winapi | 0.9.1 | MIT | https://github.com/crossterm-rs/crossterm-winapi |
| crunchy | 0.2.4 | MIT | https://github.com/eira-fransham/crunchy |
| crypto-common | 0.1.7 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| cssparser | 0.37.0 | MPL-2.0 | https://github.com/servo/rust-cssparser |
| cssparser-macros | 0.7.1 | MPL-2.0 | https://github.com/servo/rust-cssparser |
| ctor | 1.0.13 | Apache-2.0 OR MIT | https://github.com/mmastrac/linktime |
| darling | 0.24.1 | MIT | https://github.com/TedDriggs/darling |
| darling_core | 0.24.1 | MIT | https://github.com/TedDriggs/darling |
| darling_macro | 0.24.1 | MIT | https://github.com/TedDriggs/darling |
| dbus | 0.9.12 | Apache-2.0/MIT | https://github.com/diwic/dbus-rs |
| defmt | 1.2.0 | MIT OR Apache-2.0 | https://github.com/knurling-rs/defmt |
| defmt-macros | 1.2.0 | MIT OR Apache-2.0 | https://github.com/knurling-rs/defmt |
| defmt-parser | 1.1.0 | MIT OR Apache-2.0 | https://github.com/knurling-rs/defmt |
| deranged | 0.5.9 | MIT OR Apache-2.0 | https://github.com/jhpratt/deranged |
| derive_more | 2.1.1 | MIT | https://github.com/JelteF/derive_more |
| derive_more-impl | 2.1.1 | MIT | https://github.com/JelteF/derive_more |
| digest | 0.10.7 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| dirs | 7.0.0 | MIT OR Apache-2.0 | https://codeberg.org/dirs/dirs-rs |
| dirs-sys | 0.5.0 | MIT OR Apache-2.0 | https://github.com/dirs-dev/dirs-sys-rs |
| dispatch2 | 0.3.1 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| displaydoc | 0.2.7 | MIT OR Apache-2.0 | https://github.com/yaahc/displaydoc |
| dlopen2 | 0.8.2 | MIT | https://github.com/OpenByteDev/dlopen2 |
| dlopen2_derive | 0.4.3 | MIT | https://github.com/OpenByteDev/dlopen2 |
| dom_query | 0.28.0 | MIT | https://github.com/niklak/dom_query |
| dpi | 0.1.2 | Apache-2.0 AND MIT | https://github.com/rust-windowing/winit |
| dtoa | 1.0.11 | MIT OR Apache-2.0 | https://github.com/dtolnay/dtoa |
| dtoa-short | 0.3.5 | MPL-2.0 | https://github.com/upsuper/dtoa-short |
| duckdb | 1.10506.1 | MIT | https://github.com/duckdb/duckdb-rs |
| dunce | 1.0.5 | CC0-1.0 OR MIT-0 OR Apache-2.0 | https://gitlab.com/kornelski/dunce |
| dyn-clone | 1.0.20 | MIT OR Apache-2.0 | https://github.com/dtolnay/dyn-clone |
| embed-resource | 3.0.12 | MIT | https://github.com/nabijaczleweli/rust-embed-resource |
| embed_plist | 1.2.2 | MIT OR Apache-2.0 | https://github.com/nvzqz/embed-plist-rs |
| equivalent | 1.0.3 | Apache-2.0 OR MIT | https://github.com/indexmap-rs/equivalent |
| erased-serde | 0.4.10 | MIT OR Apache-2.0 | https://github.com/dtolnay/erased-serde |
| errno | 0.3.14 | MIT OR Apache-2.0 | https://github.com/lambda-fairy/rust-errno |
| fallible-iterator | 0.3.0 | MIT/Apache-2.0 | https://github.com/sfackler/rust-fallible-iterator |
| fallible-streaming-iterator | 0.1.9 | MIT/Apache-2.0 | https://github.com/sfackler/fallible-streaming-iterator |
| fastrand | 2.5.0 | Apache-2.0 OR MIT | https://github.com/smol-rs/fastrand |
| fdeflate | 0.3.7 | MIT OR Apache-2.0 | https://github.com/image-rs/fdeflate |
| field-offset | 0.3.6 | MIT OR Apache-2.0 | https://github.com/Diggsey/rust-field-offset |
| filetime | 0.2.29 | MIT/Apache-2.0 | https://github.com/alexcrichton/filetime |
| find-msvc-tools | 0.1.15 | MIT OR Apache-2.0 | https://github.com/rust-lang/cc-rs |
| flate2 | 1.1.10 | MIT OR Apache-2.0 | https://github.com/rust-lang/flate2-rs |
| fnv | 1.0.7 | Apache-2.0 / MIT | https://github.com/servo/rust-fnv |
| foldhash | 0.1.5 | Zlib | https://github.com/orlp/foldhash |
| foldhash | 0.2.0 | Zlib | https://github.com/orlp/foldhash |
| foreign-types | 0.5.0 | MIT/Apache-2.0 | https://github.com/sfackler/foreign-types |
| foreign-types-macros | 0.2.4 | MIT/Apache-2.0 | https://github.com/sfackler/foreign-types |
| foreign-types-shared | 0.3.1 | MIT/Apache-2.0 | https://github.com/sfackler/foreign-types |
| form_urlencoded | 1.2.2 | MIT OR Apache-2.0 | https://github.com/servo/rust-url |
| futures-channel | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-core | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-executor | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-io | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-macro | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-sink | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-task | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-util | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| gdal | 0.19.0 | MIT | https://github.com/georust/gdal |
| gdal-sys | 0.12.0 | MIT | https://github.com/georust/gdal |
| gdk | 0.18.2 | MIT | https://github.com/gtk-rs/gtk3-rs |
| gdk-pixbuf | 0.18.5 | MIT | https://github.com/gtk-rs/gtk-rs-core |
| gdk-pixbuf-sys | 0.18.0 | MIT | https://github.com/gtk-rs/gtk-rs-core |
| gdk-sys | 0.18.2 | MIT | https://github.com/gtk-rs/gtk3-rs |
| gdkwayland-sys | 0.18.2 | MIT | https://github.com/gtk-rs/gtk3-rs |
| gdkx11 | 0.18.2 | MIT | https://github.com/gtk-rs/gtk3-rs |
| gdkx11-sys | 0.18.2 | MIT | https://github.com/gtk-rs/gtk3-rs |
| generic-array | 0.14.7 | MIT | https://github.com/fizyk20/generic-array.git |
| geo-types | 0.7.20 | MIT OR Apache-2.0 | https://github.com/georust/geo |
| getrandom | 0.2.17 | MIT OR Apache-2.0 | https://github.com/rust-random/getrandom |
| getrandom | 0.3.4 | MIT OR Apache-2.0 | https://github.com/rust-random/getrandom |
| getrandom | 0.4.3 | MIT OR Apache-2.0 | https://github.com/rust-random/getrandom |
| gio | 0.18.4 | MIT | https://github.com/gtk-rs/gtk-rs-core |
| gio-sys | 0.18.1 | MIT | https://github.com/gtk-rs/gtk-rs-core |
| glib | 0.18.5 | MIT | https://github.com/gtk-rs/gtk-rs-core |
| glib-macros | 0.18.5 | MIT | https://github.com/gtk-rs/gtk-rs-core |
| glib-sys | 0.18.1 | MIT | https://github.com/gtk-rs/gtk-rs-core |
| glob | 0.3.4 | MIT OR Apache-2.0 | https://github.com/rust-lang/glob |
| gobject-sys | 0.18.0 | MIT | https://github.com/gtk-rs/gtk-rs-core |
| gtk | 0.18.2 | MIT | https://github.com/gtk-rs/gtk3-rs |
| gtk-sys | 0.18.2 | MIT | https://github.com/gtk-rs/gtk3-rs |
| gtk3-macros | 0.18.2 | MIT | https://github.com/gtk-rs/gtk3-rs |
| half | 2.7.1 | MIT OR Apache-2.0 | https://github.com/VoidStarKat/half-rs |
| hashbrown | 0.12.3 | MIT OR Apache-2.0 | https://github.com/rust-lang/hashbrown |
| hashbrown | 0.15.5 | MIT OR Apache-2.0 | https://github.com/rust-lang/hashbrown |
| hashbrown | 0.16.1 | MIT OR Apache-2.0 | https://github.com/rust-lang/hashbrown |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 | https://github.com/rust-lang/hashbrown |
| hashlink | 0.10.0 | MIT OR Apache-2.0 | https://github.com/kyren/hashlink |
| heck | 0.4.1 | MIT OR Apache-2.0 | https://github.com/withoutboats/heck |
| heck | 0.5.0 | MIT OR Apache-2.0 | https://github.com/withoutboats/heck |
| hex | 0.4.3 | MIT OR Apache-2.0 | https://github.com/KokaKiwi/rust-hex |
| html5ever | 0.39.0 | MIT OR Apache-2.0 | https://github.com/servo/html5ever |
| http | 1.5.0 | MIT OR Apache-2.0 | https://github.com/hyperium/http |
| http-body | 1.1.0 | MIT | https://github.com/hyperium/http-body |
| http-body-util | 0.1.5 | MIT | https://github.com/hyperium/http-body |
| httparse | 1.10.1 | MIT OR Apache-2.0 | https://github.com/seanmonstar/httparse |
| httpdate | 1.0.3 | MIT OR Apache-2.0 | https://github.com/pyfisch/httpdate |
| hyper | 1.12.0 | MIT | https://github.com/hyperium/hyper |
| hyper-util | 0.1.21 | MIT | https://github.com/hyperium/hyper-util |
| iana-time-zone | 0.1.65 | MIT OR Apache-2.0 | https://github.com/strawlab/iana-time-zone |
| iana-time-zone-haiku | 0.1.2 | MIT OR Apache-2.0 | https://github.com/strawlab/iana-time-zone |
| ico | 0.6.0 | MIT | https://github.com/mdsteele/rust-ico |
| icu_collections | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_locale_core | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_normalizer | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_normalizer_data | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_properties | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_properties_data | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_provider | 2.3.1 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| ident_case | 1.0.1 | MIT/Apache-2.0 | https://github.com/TedDriggs/ident_case |
| idna | 1.1.0 | MIT OR Apache-2.0 | https://github.com/servo/rust-url/ |
| idna_adapter | 1.2.2 | Apache-2.0 OR MIT | https://github.com/hsivonen/idna_adapter |
| indexmap | 1.9.3 | Apache-2.0 OR MIT | https://github.com/bluss/indexmap |
| indexmap | 2.14.2 | Apache-2.0 OR MIT | https://github.com/indexmap-rs/indexmap |
| infer | 0.22.0 | MIT | https://github.com/bojand/infer |
| ipnet | 2.12.2 | MIT OR Apache-2.0 | https://github.com/krisprice/ipnet |
| itoa | 1.0.18 | MIT OR Apache-2.0 | https://github.com/dtolnay/itoa |
| javascriptcore-rs | 1.1.2 | MIT | https://github.com/tauri-apps/javascriptcore-rs |
| javascriptcore-rs-sys | 1.1.1 | MIT | https://github.com/tauri-apps/javascriptcore-rs |
| jiff | 0.2.38 | Unlicense OR MIT | https://github.com/BurntSushi/jiff |
| jiff-core | 0.1.1 | Unlicense OR MIT | https://github.com/BurntSushi/jiff |
| jiff-static | 0.2.38 | Unlicense OR MIT | https://github.com/BurntSushi/jiff |
| jiff-tzdb | 0.1.9 | Unlicense OR MIT | https://github.com/BurntSushi/jiff |
| jiff-tzdb-platform | 0.1.3 | Unlicense OR MIT | https://github.com/BurntSushi/jiff |
| jni | 0.21.1 | MIT/Apache-2.0 | https://github.com/jni-rs/jni-rs |
| jni-sys | 0.3.1 | MIT OR Apache-2.0 | https://github.com/jni-rs/jni-sys |
| jni-sys | 0.4.1 | MIT OR Apache-2.0 | https://github.com/jni-rs/jni-sys |
| jni-sys-macros | 0.4.1 | MIT OR Apache-2.0 | https://github.com/jni-rs/jni-sys |
| js-sys | 0.3.106 | MIT OR Apache-2.0 | https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/js-sys |
| json-patch | 4.2.0 | MIT/Apache-2.0 | https://github.com/idubrov/json-patch |
| jsonptr | 0.7.1 | MIT OR Apache-2.0 | https://github.com/chanced/jsonptr |
| keyboard-types | 0.8.3 | MIT OR Apache-2.0 | https://github.com/rust-windowing/keyboard-types |
| lexical-core | 1.0.6 | MIT/Apache-2.0 | https://github.com/Alexhuszagh/rust-lexical |
| lexical-parse-float | 1.0.6 | MIT/Apache-2.0 | https://github.com/Alexhuszagh/rust-lexical |
| lexical-parse-integer | 1.0.6 | MIT/Apache-2.0 | https://github.com/Alexhuszagh/rust-lexical |
| lexical-util | 1.0.7 | MIT/Apache-2.0 | https://github.com/Alexhuszagh/rust-lexical |
| lexical-write-float | 1.0.6 | MIT/Apache-2.0 | https://github.com/Alexhuszagh/rust-lexical |
| lexical-write-integer | 1.0.6 | MIT/Apache-2.0 | https://github.com/Alexhuszagh/rust-lexical |
| libappindicator | 0.9.0 | Apache-2.0 OR MIT | https://crates.io/crates/libappindicator |
| libappindicator-sys | 0.9.0 | Apache-2.0 OR MIT | https://crates.io/crates/libappindicator-sys |
| libc | 0.2.191 | MIT OR Apache-2.0 | https://github.com/rust-lang/libc |
| libdbus-sys | 0.2.7 | Apache-2.0/MIT | https://github.com/diwic/dbus-rs |
| libduckdb-sys | 1.10506.1 | MIT | https://github.com/duckdb/duckdb-rs |
| libloading | 0.7.4 | ISC | https://github.com/nagisa/rust_libloading/ |
| libm | 0.2.16 | MIT | https://github.com/rust-lang/compiler-builtins |
| libredox | 0.1.25 | MIT | https://gitlab.redox-os.org/redox-os/libredox.git |
| linux-raw-sys | 0.12.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | https://github.com/sunfishcode/linux-raw-sys |
| linux-raw-sys | 0.4.15 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | https://github.com/sunfishcode/linux-raw-sys |
| litemap | 0.8.3 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| lock_api | 0.4.14 | MIT OR Apache-2.0 | https://github.com/Amanieu/parking_lot |
| log | 0.4.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/log |
| lru | 0.16.4 | MIT | https://github.com/jeromefroe/lru-rs.git |
| markup5ever | 0.39.0 | MIT OR Apache-2.0 | https://github.com/servo/html5ever |
| memchr | 2.8.3 | Unlicense OR MIT | https://github.com/BurntSushi/memchr |
| memoffset | 0.9.1 | MIT | https://github.com/Gilnaa/memoffset |
| mime | 0.3.17 | MIT OR Apache-2.0 | https://github.com/hyperium/mime |
| miniz_oxide | 0.8.9 | MIT OR Zlib OR Apache-2.0 | https://github.com/Frommi/miniz_oxide/tree/master/miniz_oxide |
| miniz_oxide | 0.9.1 | MIT OR Zlib OR Apache-2.0 | https://github.com/Frommi/miniz_oxide/tree/master/miniz_oxide |
| mio | 1.2.4 | MIT | https://github.com/tokio-rs/mio |
| muda | 0.20.0 | Apache-2.0 OR MIT | https://github.com/tauri-apps/muda |
| ndk | 0.9.0 | MIT OR Apache-2.0 | https://github.com/rust-mobile/ndk |
| ndk-context | 0.1.1 | MIT OR Apache-2.0 | https://github.com/rust-windowing/android-ndk-rs |
| ndk-sys | 0.6.0+11769913 | MIT OR Apache-2.0 | https://github.com/rust-mobile/ndk |
| new_debug_unreachable | 1.0.6 | MIT | https://github.com/mbrubeck/rust-debug-unreachable |
| nix | 0.31.3 | MIT | https://github.com/nix-rust/nix |
| num-bigint | 0.4.8 | MIT OR Apache-2.0 | https://github.com/rust-num/num-bigint |
| num-complex | 0.4.6 | MIT OR Apache-2.0 | https://github.com/rust-num/num-complex |
| num-conv | 0.2.2 | MIT OR Apache-2.0 | https://github.com/jhpratt/num-conv |
| num-integer | 0.1.47 | MIT OR Apache-2.0 | https://github.com/rust-num/num-integer |
| num-traits | 0.2.19 | MIT OR Apache-2.0 | https://github.com/rust-num/num-traits |
| num_enum | 0.7.6 | BSD-3-Clause OR MIT OR Apache-2.0 | https://github.com/illicitonion/num_enum |
| num_enum_derive | 0.7.6 | BSD-3-Clause OR MIT OR Apache-2.0 | https://github.com/illicitonion/num_enum |
| objc2 | 0.6.5 | MIT | https://github.com/madsmtm/objc2 |
| objc2-app-kit | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| objc2-cloud-kit | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| objc2-core-data | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| objc2-core-foundation | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| objc2-core-graphics | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| objc2-core-image | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| objc2-core-location | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| objc2-core-text | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| objc2-encode | 4.1.0 | MIT | https://github.com/madsmtm/objc2 |
| objc2-exception-helper | 0.1.1 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| objc2-foundation | 0.3.2 | MIT | https://github.com/madsmtm/objc2 |
| objc2-io-surface | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| objc2-quartz-core | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| objc2-ui-kit | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| objc2-user-notifications | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| objc2-web-kit | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| once_cell | 1.21.4 | MIT OR Apache-2.0 | https://github.com/matklad/once_cell |
| option-ext | 0.2.0 | MPL-2.0 | https://github.com/soc/option-ext.git |
| pango | 0.18.3 | MIT | https://github.com/gtk-rs/gtk-rs-core |
| pango-sys | 0.18.0 | MIT | https://github.com/gtk-rs/gtk-rs-core |
| parking_lot | 0.12.5 | MIT OR Apache-2.0 | https://github.com/Amanieu/parking_lot |
| parking_lot_core | 0.9.12 | MIT OR Apache-2.0 | https://github.com/Amanieu/parking_lot |
| percent-encoding | 2.3.2 | MIT OR Apache-2.0 | https://github.com/servo/rust-url/ |
| phf | 0.13.1 | MIT | https://github.com/rust-phf/rust-phf |
| phf_codegen | 0.13.1 | MIT | https://github.com/rust-phf/rust-phf |
| phf_generator | 0.13.1 | MIT | https://github.com/rust-phf/rust-phf |
| phf_macros | 0.13.1 | MIT | https://github.com/rust-phf/rust-phf |
| phf_shared | 0.13.1 | MIT | https://github.com/rust-phf/rust-phf |
| pin-project-lite | 0.2.17 | Apache-2.0 OR MIT | https://github.com/taiki-e/pin-project-lite |
| pkg-config | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/pkg-config-rs |
| plist | 1.10.3 | MIT | https://github.com/ebarnard/rust-plist/ |
| png | 0.18.1 | MIT OR Apache-2.0 | https://github.com/image-rs/image-png |
| portable-atomic | 1.15.0 | Apache-2.0 OR MIT | https://github.com/taiki-e/portable-atomic |
| portable-atomic-util | 0.2.8 | Apache-2.0 OR MIT | https://github.com/taiki-e/portable-atomic-util |
| potential_utf | 0.1.6 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| powerfmt | 0.2.1 | MIT OR Apache-2.0 | https://github.com/jhpratt/powerfmt |
| precomputed-hash | 0.1.1 | MIT | https://github.com/emilio/precomputed-hash |
| proc-macro-crate | 1.3.1 | MIT OR Apache-2.0 | https://github.com/bkchr/proc-macro-crate |
| proc-macro-crate | 2.0.2 | MIT OR Apache-2.0 | https://github.com/bkchr/proc-macro-crate |
| proc-macro-crate | 3.5.0 | MIT OR Apache-2.0 | https://github.com/bkchr/proc-macro-crate |
| proc-macro-error | 1.0.4 | MIT OR Apache-2.0 | https://gitlab.com/CreepySkeleton/proc-macro-error |
| proc-macro-error-attr | 1.0.4 | MIT OR Apache-2.0 | https://gitlab.com/CreepySkeleton/proc-macro-error |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 | https://github.com/dtolnay/proc-macro2 |
| quick-xml | 0.42.0 | MIT | https://github.com/tafia/quick-xml |
| quote | 1.0.47 | MIT OR Apache-2.0 | https://github.com/dtolnay/quote |
| r-efi | 5.3.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later | https://github.com/r-efi/r-efi |
| r-efi | 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later | https://github.com/r-efi/r-efi |
| raw-window-handle | 0.6.2 | MIT OR Apache-2.0 OR Zlib | https://github.com/rust-windowing/raw-window-handle |
| redox_syscall | 0.5.18 | MIT | https://gitlab.redox-os.org/redox-os/syscall |
| redox_users | 0.5.3 | MIT | https://gitlab.redox-os.org/redox-os/users |
| ref-cast | 1.0.27 | MIT OR Apache-2.0 | https://github.com/dtolnay/ref-cast |
| ref-cast-impl | 1.0.27 | MIT OR Apache-2.0 | https://github.com/dtolnay/ref-cast |
| regex | 1.13.1 | MIT OR Apache-2.0 | https://github.com/rust-lang/regex |
| regex-automata | 0.4.18 | MIT OR Apache-2.0 | https://github.com/rust-lang/regex |
| regex-syntax | 0.8.11 | MIT OR Apache-2.0 | https://github.com/rust-lang/regex |
| reqwest | 0.13.5 | MIT OR Apache-2.0 | https://github.com/seanmonstar/reqwest |
| rfd | 0.16.0 | MIT | https://github.com/PolyMeilex/rfd |
| ring | 0.17.14 | Apache-2.0 AND ISC | https://github.com/briansmith/ring |
| rustc-hash | 2.1.3 | Apache-2.0 OR MIT | https://github.com/rust-lang/rustc-hash |
| rustc_version | 0.4.1 | MIT OR Apache-2.0 | https://github.com/djc/rustc-version-rs |
| rustix | 0.38.44 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | https://github.com/bytecodealliance/rustix |
| rustix | 1.1.5 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | https://github.com/bytecodealliance/rustix |
| rustls | 0.23.46 | Apache-2.0 OR ISC OR MIT | https://github.com/rustls/rustls |
| rustls-pki-types | 1.15.1 | MIT OR Apache-2.0 | https://github.com/rustls/pki-types |
| rustls-webpki | 0.103.16 | ISC | https://github.com/rustls/webpki |
| rustversion | 1.0.23 | MIT OR Apache-2.0 | https://github.com/dtolnay/rustversion |
| ryu | 1.0.23 | Apache-2.0 OR BSL-1.0 | https://github.com/dtolnay/ryu |
| same-file | 1.0.6 | Unlicense/MIT | https://github.com/BurntSushi/same-file |
| schemars | 0.8.22 | MIT | https://github.com/GREsau/schemars |
| schemars | 0.9.0 | MIT | https://github.com/GREsau/schemars |
| schemars | 1.2.3 | MIT | https://github.com/GREsau/schemars |
| schemars_derive | 0.8.22 | MIT | https://github.com/GREsau/schemars |
| scopeguard | 1.2.0 | MIT OR Apache-2.0 | https://github.com/bluss/scopeguard |
| selectors | 0.38.0 | MPL-2.0 | https://github.com/servo/stylo |
| semver | 1.0.28 | MIT OR Apache-2.0 | https://github.com/dtolnay/semver |
| serde | 1.0.229 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| serde-untagged | 0.1.9 | MIT OR Apache-2.0 | https://github.com/dtolnay/serde-untagged |
| serde_core | 1.0.229 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| serde_derive_internals | 0.29.1 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| serde_json | 1.0.154 | MIT OR Apache-2.0 | https://github.com/serde-rs/json |
| serde_repr | 0.1.21 | MIT OR Apache-2.0 | https://github.com/dtolnay/serde-repr |
| serde_spanned | 0.6.9 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| serde_spanned | 1.1.2 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| serde_with | 3.24.0 | MIT OR Apache-2.0 | https://github.com/jonasbb/serde_with/ |
| serde_with_macros | 3.24.0 | MIT OR Apache-2.0 | https://github.com/jonasbb/serde_with/ |
| serialize-to-javascript | 0.1.2 | MIT OR Apache-2.0 | https://github.com/chippers/serialize-to-javascript |
| serialize-to-javascript-impl | 0.1.2 | MIT OR Apache-2.0 | https://github.com/chippers/serialize-to-javascript |
| servo_arc | 0.4.3 | MIT OR Apache-2.0 | https://github.com/servo/stylo |
| sha2 | 0.10.9 | MIT OR Apache-2.0 | https://github.com/RustCrypto/hashes |
| shlex | 2.0.1 | MIT OR Apache-2.0 | https://github.com/comex/rust-shlex |
| simd-adler32 | 0.3.10 | MIT | https://github.com/mcountryman/simd-adler32 |
| siphasher | 1.0.4 | MIT OR Apache-2.0 | https://github.com/jedisct1/rust-siphash |
| slab | 0.4.12 | MIT | https://github.com/tokio-rs/slab |
| smallvec | 1.16.3 | MIT OR Apache-2.0 | https://github.com/servo/rust-smallvec |
| socket2 | 0.6.5 | MIT OR Apache-2.0 | https://github.com/rust-lang/socket2 |
| softbuffer | 0.4.8 | MIT OR Apache-2.0 | https://github.com/rust-windowing/softbuffer |
| soup3 | 0.5.0 | MIT | https://gitlab.gnome.org/World/Rust/soup3-rs |
| soup3-sys | 0.5.0 | MIT | https://gitlab.gnome.org/World/Rust/soup3-rs |
| stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 | https://github.com/storyyeller/stable_deref_trait |
| static_vcruntime | 3.0.0 | MIT OR Apache-2.0 OR Zlib | https://github.com/ChrisDenton/static_vcruntime |
| string_cache | 0.9.0 | MIT OR Apache-2.0 | https://github.com/servo/string-cache |
| string_cache_codegen | 0.6.1 | MIT OR Apache-2.0 | https://github.com/servo/string-cache |
| strsim | 0.11.1 | MIT | https://github.com/rapidfuzz/strsim-rs |
| strum | 0.27.2 | MIT | https://github.com/Peternator7/strum |
| strum_macros | 0.27.2 | MIT | https://github.com/Peternator7/strum |
| subtle | 2.6.1 | BSD-3-Clause | https://github.com/dalek-cryptography/subtle |
| swift-rs | 1.0.9 | MIT OR Apache-2.0 | https://github.com/Brendonovich/swift-rs |
| syn | 1.0.109 | MIT OR Apache-2.0 | https://github.com/dtolnay/syn |
| syn | 2.0.119 | MIT OR Apache-2.0 | https://github.com/dtolnay/syn |
| syn | 3.0.9 | MIT OR Apache-2.0 | https://github.com/dtolnay/syn |
| sync_wrapper | 1.0.2 | Apache-2.0 | https://github.com/Actyx/sync_wrapper |
| synstructure | 0.14.0 | MIT | https://github.com/mystor/synstructure |
| system-deps | 6.2.2 | MIT OR Apache-2.0 | https://github.com/gdesmott/system-deps |
| tao | 0.37.1 | Apache-2.0 | https://github.com/tauri-apps/tao |
| tao-macros | 0.1.4 | MIT OR Apache-2.0 | https://github.com/tauri-apps/tao |
| tar | 0.4.46 | MIT OR Apache-2.0 | https://github.com/composefs/tar-rs |
| target-lexicon | 0.12.16 | Apache-2.0 WITH LLVM-exception | https://github.com/bytecodealliance/target-lexicon |
| tauri | 2.12.5 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri-build | 2.7.4 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri-codegen | 2.7.4 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri-macros | 2.7.4 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri-plugin | 2.7.2 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri-plugin-dialog | 2.8.3 | Apache-2.0 OR MIT | https://github.com/tauri-apps/plugins-workspace |
| tauri-plugin-fs | 2.7.1 | Apache-2.0 OR MIT | https://github.com/tauri-apps/plugins-workspace |
| tauri-runtime | 2.12.2 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri-runtime-wry | 2.12.3 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri-utils | 2.10.2 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri-winres | 0.3.7 | MIT | https://github.com/tauri-apps/winres |
| tempfile | 3.27.0 | MIT OR Apache-2.0 | https://github.com/Stebalien/tempfile |
| tendril | 0.5.1 | MIT OR Apache-2.0 | https://github.com/servo/html5ever |
| thiserror | 1.0.69 | MIT OR Apache-2.0 | https://github.com/dtolnay/thiserror |
| thiserror | 2.0.21 | MIT OR Apache-2.0 | https://github.com/dtolnay/thiserror |
| thiserror-impl | 1.0.69 | MIT OR Apache-2.0 | https://github.com/dtolnay/thiserror |
| thiserror-impl | 2.0.21 | MIT OR Apache-2.0 | https://github.com/dtolnay/thiserror |
| time | 0.3.55 | MIT OR Apache-2.0 | https://github.com/time-rs/time |
| time-core | 0.1.9 | MIT OR Apache-2.0 | https://github.com/time-rs/time |
| time-macros | 0.2.32 | MIT OR Apache-2.0 | https://github.com/time-rs/time |
| tiny-keccak | 2.0.2 | CC0-1.0 | https://github.com/debris/tiny-keccak |
| tiny_http | 0.12.0 | MIT OR Apache-2.0 | https://github.com/tiny-http/tiny-http |
| tinystr | 0.8.4 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| tinyvec | 1.13.3 | Zlib OR Apache-2.0 OR MIT | https://github.com/Lokathor/tinyvec |
| tokio | 1.54.1 | MIT | https://github.com/tokio-rs/tokio |
| tokio-util | 0.7.20 | MIT | https://github.com/tokio-rs/tokio |
| toml | 0.8.2 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| toml | 1.1.8+spec-1.1.0 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| toml_datetime | 0.6.3 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| toml_datetime | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| toml_edit | 0.19.15 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| toml_edit | 0.20.2 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| toml_edit | 0.25.17+spec-1.1.0 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| toml_parser | 1.1.5+spec-1.1.0 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| toml_writer | 1.1.3+spec-1.1.0 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| tower | 0.5.3 | MIT | https://github.com/tower-rs/tower |
| tower-http | 0.6.11 | MIT | https://github.com/tower-rs/tower-http |
| tower-layer | 0.3.3 | MIT | https://github.com/tower-rs/tower |
| tower-service | 0.3.3 | MIT | https://github.com/tower-rs/tower |
| tracing | 0.1.44 | MIT | https://github.com/tokio-rs/tracing |
| tracing-core | 0.1.36 | MIT | https://github.com/tokio-rs/tracing |
| tray-icon | 0.25.1 | MIT OR Apache-2.0 | https://github.com/tauri-apps/tray-icon |
| try-lock | 0.2.5 | MIT | https://github.com/seanmonstar/try-lock |
| typed-path | 0.12.4 | MIT OR Apache-2.0 | https://github.com/chipsenkbeil/typed-path |
| typeid | 1.0.3 | MIT OR Apache-2.0 | https://github.com/dtolnay/typeid |
| typenum | 1.20.1 | MIT OR Apache-2.0 | https://github.com/paholg/typenum |
| unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 | https://github.com/dtolnay/unicode-ident |
| unicode-segmentation | 1.13.3 | MIT OR Apache-2.0 | https://github.com/unicode-rs/unicode-segmentation |
| unicode-width | 0.2.2 | MIT OR Apache-2.0 | https://github.com/unicode-rs/unicode-width |
| untrusted | 0.9.0 | ISC | https://github.com/briansmith/untrusted |
| ureq | 2.12.1 | MIT OR Apache-2.0 | https://github.com/algesten/ureq |
| ureq | 3.5.0 | MIT OR Apache-2.0 | https://github.com/algesten/ureq |
| ureq-proto | 0.6.5 | MIT OR Apache-2.0 | https://github.com/algesten/ureq-proto |
| url | 2.5.8 | MIT OR Apache-2.0 | https://github.com/servo/rust-url |
| urlpattern | 0.6.0 | MIT | https://github.com/denoland/rust-urlpattern |
| utf8-zero | 0.8.1 | MIT OR Apache-2.0 | https://github.com/algesten/utf8-zero |
| utf8_iter | 1.0.4 | Apache-2.0 OR MIT | https://github.com/hsivonen/utf8_iter |
| uuid | 1.28.0 | Apache-2.0 OR MIT | https://github.com/uuid-rs/uuid |
| vcpkg | 0.2.15 | MIT/Apache-2.0 | https://github.com/mcgoo/vcpkg-rs |
| version-compare | 0.2.1 | MIT | https://gitlab.com/timvisee/version-compare |
| version_check | 0.9.5 | MIT/Apache-2.0 | https://github.com/SergioBenitez/version_check |
| vswhom | 0.1.0 | MIT | https://github.com/nabijaczleweli/vswhom.rs |
| vswhom-sys | 0.1.3 | MIT | https://github.com/nabijaczleweli/vswhom-sys.rs |
| walkdir | 2.5.0 | Unlicense/MIT | https://github.com/BurntSushi/walkdir |
| want | 0.3.2 | MIT | https://github.com/seanmonstar/want |
| wasi | 0.11.1+wasi-snapshot-preview1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | https://github.com/bytecodealliance/wasi |
| wasip2 | 1.0.4+wasi-0.2.12 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | https://github.com/bytecodealliance/wasi-rs |
| wasm-bindgen | 0.2.129 | MIT OR Apache-2.0 | https://github.com/wasm-bindgen/wasm-bindgen |
| wasm-bindgen-futures | 0.4.79 | MIT OR Apache-2.0 | https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/futures |
| wasm-bindgen-macro | 0.2.129 | MIT OR Apache-2.0 | https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/macro |
| wasm-bindgen-macro-support | 0.2.129 | MIT OR Apache-2.0 | https://github.com/wasm-bindgen/wasm-bindgen/tree/main/crates/macro-support |
| wasm-bindgen-shared | 0.2.129 | MIT OR Apache-2.0 | https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/shared |
| wasm-streams | 0.5.0 | MIT OR Apache-2.0 | https://github.com/MattiasBuelens/wasm-streams/ |
| web-sys | 0.3.106 | MIT OR Apache-2.0 | https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/web-sys |
| web-time | 1.1.0 | MIT OR Apache-2.0 | https://github.com/daxpedda/web-time |
| web_atoms | 0.2.6 | MIT OR Apache-2.0 | https://github.com/servo/html5ever |
| webkit2gtk | 2.0.2 | MIT | https://github.com/tauri-apps/webkit2gtk-rs |
| webkit2gtk-sys | 2.0.2 | MIT | https://github.com/tauri-apps/webkit2gtk-rs |
| webpki-roots | 0.26.11 | CDLA-Permissive-2.0 | https://github.com/rustls/webpki-roots |
| webpki-roots | 1.0.10 | CDLA-Permissive-2.0 | https://github.com/rustls/webpki-roots |
| webview2-com | 0.39.1 | MIT | https://github.com/wravery/webview2-rs |
| webview2-com-macros | 0.8.1 | MIT | https://github.com/wravery/webview2-rs |
| webview2-com-sys | 0.39.1 | MIT | https://github.com/wravery/webview2-rs |
| winapi | 0.3.9 | MIT/Apache-2.0 | https://github.com/retep998/winapi-rs |
| winapi-i686-pc-windows-gnu | 0.4.0 | MIT/Apache-2.0 | https://github.com/retep998/winapi-rs |
| winapi-util | 0.1.11 | Unlicense OR MIT | https://github.com/BurntSushi/winapi-util |
| winapi-x86_64-pc-windows-gnu | 0.4.0 | MIT/Apache-2.0 | https://github.com/retep998/winapi-rs |
| window-vibrancy | 0.8.1 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri-plugin-vibrancy |
| windows | 0.62.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-collections | 0.3.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-core | 0.62.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-future | 0.3.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-implement | 0.60.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-interface | 0.59.3 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-link | 0.2.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-numerics | 0.3.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-result | 0.4.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-strings | 0.5.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-sys | 0.45.0 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-sys | 0.52.0 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-sys | 0.59.0 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-sys | 0.60.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-targets | 0.42.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-targets | 0.52.6 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-targets | 0.53.5 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-threading | 0.2.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-version | 0.1.7 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_aarch64_gnullvm | 0.42.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_aarch64_gnullvm | 0.52.6 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_aarch64_gnullvm | 0.53.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_aarch64_msvc | 0.42.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_aarch64_msvc | 0.52.6 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_aarch64_msvc | 0.53.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_i686_gnu | 0.42.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_i686_gnu | 0.52.6 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_i686_gnu | 0.53.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_i686_gnullvm | 0.52.6 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_i686_gnullvm | 0.53.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_i686_msvc | 0.42.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_i686_msvc | 0.52.6 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_i686_msvc | 0.53.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_x86_64_gnu | 0.42.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_x86_64_gnu | 0.52.6 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_x86_64_gnu | 0.53.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_x86_64_gnullvm | 0.42.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_x86_64_gnullvm | 0.52.6 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_x86_64_gnullvm | 0.53.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_x86_64_msvc | 0.42.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_x86_64_msvc | 0.52.6 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_x86_64_msvc | 0.53.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| winnow | 0.5.40 | MIT | https://github.com/winnow-rs/winnow |
| winnow | 1.0.4 | MIT | https://github.com/winnow-rs/winnow |
| winreg | 0.56.0 | MIT | https://github.com/gentoo90/winreg-rs |
| wit-bindgen | 0.57.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | https://github.com/bytecodealliance/wit-bindgen |
| writeable | 0.6.4 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| wry | 0.57.0 | Apache-2.0 OR MIT | https://github.com/tauri-apps/wry |
| x11 | 2.21.0 | MIT | https://github.com/AltF02/x11-rs.git |
| x11-dl | 2.21.0 | MIT | https://github.com/AltF02/x11-rs.git |
| xattr | 1.6.1 | MIT OR Apache-2.0 | https://github.com/Stebalien/xattr |
| yoke | 0.8.3 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| yoke-derive | 0.8.4 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zerocopy | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT | https://github.com/google/zerocopy |
| zerocopy-derive | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT | https://github.com/google/zerocopy |
| zerofrom | 0.1.8 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zerofrom-derive | 0.1.8 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zeroize | 1.9.1 | Apache-2.0 OR MIT | https://github.com/RustCrypto/utils |
| zerotrie | 0.2.5 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zerovec | 0.11.8 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zerovec-derive | 0.11.6 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zip | 9.0.5 | MIT | https://github.com/zip-rs/zip2 |
| zlib-rs | 0.6.8 | Zlib | https://github.com/trifectatechfoundation/zlib-rs |
| zmij | 1.0.23 | MIT | https://github.com/dtolnay/zmij |
| zopfli | 0.8.4 | Apache-2.0 | https://github.com/zopfli-rs/zopfli |
