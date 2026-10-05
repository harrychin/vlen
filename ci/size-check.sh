#!/usr/bin/env bash
# Builds the size-check staticlib for a Cortex-M target at opt-level
# "z" with fat LTO and panic = "abort", once per API surface, and fails
# if the codec's code grows past its budget, imports memcpy, or keeps
# any panic machinery.
#
# The budgets are this harness's measurements plus ~15% for compiler
# drift; they are not the README's figures, which came from a
# different harness. Both surfaces measured the same from 0.4.3 to
# this change (280 and 774 bytes on Rust 1.97).
#
# Usage: ci/size-check.sh
# Needs the thumbv7em-none-eabi target, plus llvm-size and llvm-nm on
# PATH, in LLVM_BIN, or from `rustup component add llvm-tools`.
set -euo pipefail

cd "$(dirname "$0")/size-check"
target=thumbv7em-none-eabi
if [[ -z ${LLVM_BIN:-} ]] && ! command -v llvm-size >/dev/null; then
	LLVM_BIN="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/^host: //p')/bin"
fi
bin=${LLVM_BIN:+$LLVM_BIN/}
status=0

check() {
	local surface=$1 budget=$2
	cargo build --quiet --release --target "$target" --features "$surface"
	local lib="target/$target/release/libvlen_size_check.a"
	# With fat LTO the crate and its dependencies are one archive member;
	# compiler_builtins stays separate and only matters if referenced.
	local text
	text=$("${bin}llvm-size" -A "$lib" 2>/dev/null | awk '
		/\(ex / { inside = ($1 ~ /^vlen_size_check-/); next }
		inside && $1 ~ /^\.text/ { sum += $2 }
		END { print sum + 0 }')
	# Symbols the codec's member needs from elsewhere (memcpy and the
	# like), and any panic machinery it kept.
	local member='/^vlen_size_check-.*:$/ { inside = 1; next } /:$/ { inside = 0 }'
	local undefined panics
	undefined=$("${bin}llvm-nm" -u "$lib" 2>/dev/null | awk "$member inside && \$2 != \"\" { print \$2 }" | sort -u | tr '\n' ' ')
	undefined=${undefined% }
	panics=$("${bin}llvm-nm" -C --defined-only "$lib" 2>/dev/null | awk "$member inside" | grep -c -E 'panic|unwrap_failed|slice_.*_fail' || true)
	printf '%-10s %4d bytes of code (budget %d)  undefined: [%s]  panic symbols: %d\n' \
		"$surface" "$text" "$budget" "$undefined" "$panics"
	if (( text == 0 || text > budget )) || [[ -n $undefined ]] || (( panics > 0 )); then
		echo "  FAILED: $surface exceeds its budget, imports symbols, or keeps panic paths" >&2
		status=1
	fi
}

check unchecked 320
check checked 896
exit $status
