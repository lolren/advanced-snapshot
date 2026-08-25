#!/bin/sh
set -eu

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
	printf 'usage: %s ADVANCED_SNAPSHOT_APK [SNAPSHOT_APK]\n' "$0" >&2
	exit 2
fi

advanced_apk=$1
snapshot_apk=${2-}

for command_name in \
	appstreamcli \
	desktop-file-validate \
	diff \
	file \
	find \
	glib-compile-schemas \
	gresource \
	readelf \
	sha256sum \
	strings \
	tar
do
	if ! command -v "$command_name" >/dev/null 2>&1; then
		printf 'missing validation command: %s\n' "$command_name" >&2
		exit 2
	fi
done

if [ ! -f "$advanced_apk" ]; then
	printf 'APK does not exist: %s\n' "$advanced_apk" >&2
	exit 2
fi

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
grep -qx 'pkgver = 0.1.0-r0' "$root_dir/.PKGINFO"
file "$root_dir/usr/bin/advanced-snapshot" | grep -q 'ARM aarch64'
file "$root_dir/usr/libexec/advanced-snapshot-focus-control" | grep -q 'ARM aarch64'
readelf -h "$root_dir/usr/bin/advanced-snapshot" | grep -q 'Machine:.*AArch64'

gresource list "$root_dir/usr/share/advanced-snapshot/resources.gresource" \
	> "$work_dir/resources.actual"
if grep -Ev '^/io/github/lolren/AdvancedSnapshot/' "$work_dir/resources.actual"; then
	printf 'resource outside the Advanced Snapshot namespace\n' >&2
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

sha256sum "$advanced_apk"
printf 'Advanced Snapshot APK validation passed\n'
