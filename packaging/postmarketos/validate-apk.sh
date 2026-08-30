#!/bin/sh
set -eu

if [ "$#" -lt 1 ] || [ "$#" -gt 3 ]; then
	printf 'usage: %s ADVANCED_SNAPSHOT_APK [SNAPSHOT_APK [LANG_APK]]\n' "$0" >&2
	exit 2
fi

advanced_apk=$1
snapshot_apk=${2-}
lang_apk=${3-}
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
key_dir=${APK_KEY_DIR:-$script_dir/../keys}
key_file=${APK_KEY_FILE:-$key_dir/pmos@local-6a8b0868.rsa.pub}
apk_verify_tool=${APK_VERIFY_TOOL:-apk}

for command_name in \
	appstreamcli \
	awk \
	desktop-file-validate \
	diff \
	file \
	find \
	glib-compile-schemas \
	gresource \
	readelf \
	sha256sum \
	strings \
	tar \
	xmllint
do
	if ! command -v "$command_name" >/dev/null 2>&1; then
		printf 'missing validation command: %s\n' "$command_name" >&2
		exit 2
	fi
done

expected_version=$(
	awk -F '=' '
		$1 == "pkgver" { pkgver=$2; pkgver_count++ }
		$1 == "pkgrel" { pkgrel=$2; pkgrel_count++ }
		END {
			if (pkgver_count != 1 || pkgrel_count != 1 ||
			    pkgver == "" || pkgrel !~ /^[0-9]+$/)
				exit 1
			printf "%s-r%s", pkgver, pkgrel
		}
	' "$script_dir/APKBUILD"
) || {
	printf 'could not derive the expected package version from APKBUILD\n' >&2
	exit 2
}

if ! command -v "$apk_verify_tool" >/dev/null 2>&1; then
	printf 'missing APK verification tool: %s\n' "$apk_verify_tool" >&2
	printf 'set APK_VERIFY_TOOL to an apk or apk.static executable\n' >&2
	exit 2
fi

if [ ! -f "$key_file" ]; then
	printf 'missing packaged public verification key\n' >&2
	exit 2
fi

if [ ! -f "$advanced_apk" ]; then
	printf 'APK does not exist: %s\n' "$advanced_apk" >&2
	exit 2
fi

"$apk_verify_tool" --keys-dir "$key_dir" verify "$advanced_apk"

work_dir=$(mktemp -d)
trap 'rm -rf -- "$work_dir"' EXIT HUP INT TERM
root_dir=$work_dir/root
mkdir -p "$root_dir"

tar -tf "$advanced_apk" 2>/dev/null \
	| grep -Ev '(^\.|/$)' \
	| LC_ALL=C sort > "$work_dir/files.actual"

cat > "$work_dir/files.expected" <<'EOF'
usr/bin/advanced-snapshot
usr/libexec/advanced-snapshot-focus-control
usr/libexec/advanced-snapshot-hdr
usr/share/advanced-snapshot/resources.gresource
usr/share/applications/io.github.lolren.AdvancedSnapshot.desktop
usr/share/dbus-1/services/io.github.lolren.AdvancedSnapshot.service
usr/share/glib-2.0/schemas/io.github.lolren.AdvancedSnapshot.gschema.xml
usr/share/icons/hicolor/scalable/apps/io.github.lolren.AdvancedSnapshot.svg
usr/share/icons/hicolor/symbolic/apps/io.github.lolren.AdvancedSnapshot-symbolic.svg
usr/share/metainfo/io.github.lolren.AdvancedSnapshot.metainfo.xml
EOF

diff -u "$work_dir/files.expected" "$work_dir/files.actual"
tar -xf "$advanced_apk" -C "$root_dir" 2>/dev/null

grep -qx 'pkgname = advanced-snapshot' "$root_dir/.PKGINFO"
grep -Fqx "pkgver = $expected_version" "$root_dir/.PKGINFO"
file "$root_dir/usr/bin/advanced-snapshot" | grep -q 'ARM aarch64'
file "$root_dir/usr/libexec/advanced-snapshot-focus-control" | grep -q 'ARM aarch64'
file "$root_dir/usr/libexec/advanced-snapshot-hdr" | grep -q 'ARM aarch64'
readelf -h "$root_dir/usr/bin/advanced-snapshot" | grep -q 'Machine:.*AArch64'
readelf -h "$root_dir/usr/libexec/advanced-snapshot-hdr" | grep -q 'Machine:.*AArch64'

gresource list "$root_dir/usr/share/advanced-snapshot/resources.gresource" \
	> "$work_dir/resources.actual"
if grep -Ev '^/io/github/lolren/AdvancedSnapshot/' "$work_dir/resources.actual"; then
	printf 'resource outside the Advanced Snapshot namespace\n' >&2
	exit 1
fi

camera_ui=$work_dir/camera.ui
gresource extract "$root_dir/usr/share/advanced-snapshot/resources.gresource" \
	/io/github/lolren/AdvancedSnapshot/ui/camera.ui > "$camera_ui"
grep -Fq 'id="image_controls_toolbar"' "$camera_ui"
grep -Fq 'id="image_controls_overlay_button"' "$camera_ui"
grep -Fq 'win.image-controls' "$camera_ui"
grep -Fq 'Tap preview to focus' "$camera_ui"
zoom_in_toolbar=$(
	xmllint --xpath \
		'count(//object[@id="image_controls_toolbar"]/child/object[@id="zoom_reset_button"])' \
		"$camera_ui"
)
zoom_button_count=$(
	xmllint --xpath 'count(//object[@id="zoom_reset_button"])' "$camera_ui"
)
if [ "$zoom_in_toolbar" != 1 ] || [ "$zoom_button_count" != 1 ]; then
	printf 'zoom reset chip is not contained in the safe toolbar area\n' >&2
	exit 1
fi

calibration_ui=$work_dir/calibration.ui
gresource extract "$root_dir/usr/share/advanced-snapshot/resources.gresource" \
	/io/github/lolren/AdvancedSnapshot/ui/calibration.ui > "$calibration_ui"
grep -Fq '<property name="content-width">340</property>' "$calibration_ui"
spin_row_count=$(
	xmllint --xpath \
		'count(//object[@class="AdwSpinRow" and starts-with(@id, "ccm_")])' \
		"$calibration_ui"
)
if [ "$spin_row_count" != 9 ]; then
	printf 'camera calibration matrix must contain nine mobile-width spin rows\n' >&2
	exit 1
fi
wide_row_count=$(
	xmllint --xpath 'count(//object[@id="ccm_red_row"])' "$calibration_ui"
)
if [ "$wide_row_count" != 0 ]; then
	printf 'wide three-column calibration matrix layout found\n' >&2
	exit 1
fi

desktop_file=$root_dir/usr/share/applications/io.github.lolren.AdvancedSnapshot.desktop
service_file=$root_dir/usr/share/dbus-1/services/io.github.lolren.AdvancedSnapshot.service
metainfo_file=$root_dir/usr/share/metainfo/io.github.lolren.AdvancedSnapshot.metainfo.xml
schema_file=$root_dir/usr/share/glib-2.0/schemas/io.github.lolren.AdvancedSnapshot.gschema.xml

desktop-file-validate "$desktop_file"
grep -qx 'Exec=advanced-snapshot' "$desktop_file"
grep -qx 'Name=io.github.lolren.AdvancedSnapshot' "$service_file"
grep -qx 'Exec=/usr/bin/advanced-snapshot --gapplication-service' "$service_file"
appstreamcli validate --no-net "$metainfo_file"

mkdir -p "$work_dir/schema-check"
cp "$schema_file" "$work_dir/schema-check/"
glib-compile-schemas --strict --dry-run "$work_dir/schema-check"

if find "$root_dir/usr" -type f -exec strings {} + \
	| grep -Eq '/org/gnome/Snapshot|org\.gnome\.Snapshot|(^|/)snapshot-focus-control([^A-Za-z0-9_-]|$)'
then
	printf 'old GNOME Snapshot runtime identifier found\n' >&2
	exit 1
fi

if [ -n "$snapshot_apk" ]; then
	if [ ! -f "$snapshot_apk" ]; then
		printf 'Snapshot APK does not exist: %s\n' "$snapshot_apk" >&2
		exit 2
	fi
	tar -tf "$snapshot_apk" 2>/dev/null \
		| grep -Ev '(^\.|/$)' \
		| LC_ALL=C sort > "$work_dir/snapshot-files"
	comm -12 "$work_dir/files.actual" "$work_dir/snapshot-files" \
		> "$work_dir/overlap"
	if [ -s "$work_dir/overlap" ]; then
		printf 'file ownership overlap with GNOME Snapshot:\n' >&2
		cat "$work_dir/overlap" >&2
		exit 1
	fi
fi

if [ -n "$lang_apk" ]; then
	if [ ! -f "$lang_apk" ]; then
		printf 'language APK does not exist: %s\n' "$lang_apk" >&2
		exit 2
	fi

	"$apk_verify_tool" --keys-dir "$key_dir" verify "$lang_apk"

	lang_root=$work_dir/lang-root
	mkdir -p "$lang_root"
	tar -tf "$lang_apk" 2>/dev/null \
		| grep -Ev '(^\.|/$)' \
		| LC_ALL=C sort > "$work_dir/lang-files"
	if [ ! -s "$work_dir/lang-files" ]; then
		printf 'language APK has no translation payload\n' >&2
		exit 1
	fi
	if grep -Ev '^usr/share/locale/[^/]+/LC_MESSAGES/advanced-snapshot\.mo$' \
		"$work_dir/lang-files" > "$work_dir/lang-unexpected"
	then
		printf 'unexpected file in language APK:\n' >&2
		cat "$work_dir/lang-unexpected" >&2
		exit 1
	fi

	tar -xf "$lang_apk" -C "$lang_root" 2>/dev/null
	grep -qx 'pkgname = advanced-snapshot-lang' "$lang_root/.PKGINFO"
	grep -Fqx "pkgver = $expected_version" "$lang_root/.PKGINFO"
	grep -qx 'arch = noarch' "$lang_root/.PKGINFO"
	grep -qx 'origin = advanced-snapshot' "$lang_root/.PKGINFO"
	grep -Fqx "install_if = advanced-snapshot=$expected_version lang" \
		"$lang_root/.PKGINFO"

	comm -12 "$work_dir/files.actual" "$work_dir/lang-files" \
		> "$work_dir/lang-overlap"
	if [ -s "$work_dir/lang-overlap" ]; then
		printf 'file ownership overlap with language APK:\n' >&2
		cat "$work_dir/lang-overlap" >&2
		exit 1
	fi

	sha256sum "$lang_apk"
	printf 'Advanced Snapshot language APK validation passed\n'
fi

sha256sum "$advanced_apk"
printf 'Advanced Snapshot APK validation passed\n'
