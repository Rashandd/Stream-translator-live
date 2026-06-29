#!/usr/bin/env bash
# Her yeni session'da çalıştırılır.
# Gösterir: nerede kaldık, task ilerleme, git durumu, test sonuçları.

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

ROADMAP="streamdubber_roadmap_final.md"

echo "========================================="
echo "  StreamDubber — Session Başlangıç"
echo "========================================="

# --- 1. NEREDE KALDIK ---
echo ""
echo "[ NEREDE KALDIK ]"
if [ -f "docs/SESSION.md" ]; then
  grep -A 3 "## Neredeyim" docs/SESSION.md 2>/dev/null | grep -v "##" | grep -v "^$" | head -3 || echo "(boş)"
fi

echo ""
echo "[ SON AGENT NOTU — Claude ]"
if [ -f "docs/sessions/claude.md" ]; then
  grep -A 3 "## Sonraki Adim" docs/sessions/claude.md 2>/dev/null | grep -v "##" | grep -v "^$" | head -3 || echo "(boş)"
fi

# --- 2. TASK İLERLEME ---
echo ""
echo "[ FAZ 1 İLERLEME ]"
if [ -f "$ROADMAP" ]; then
  python3 - "$ROADMAP" <<'PYEOF'
import sys, re

roadmap = open(sys.argv[1]).read()

# Faz 1 bölümünü çıkar
m = re.search(r'(## Faz 1.*?)(## Faz 2)', roadmap, re.DOTALL)
if not m:
    print("  Roadmap parse edilemedi")
    sys.exit(0)

section = m.group(1)
all_tasks  = re.findall(r'- \[[x ]\]', section)
done_tasks = re.findall(r'- \[x\]', section)
open_tasks = re.findall(r'- \[ \]', section)

total    = len(all_tasks)
done     = len(done_tasks)
remaining = total - done

print(f"  Tamamlanan : {done} / {total}")
print(f"  Kalan      : {remaining} task")

print("\n  Son tamamlananlar:")
done_lines = [l.strip() for l in section.splitlines() if '- [x]' in l]
if done_lines:
    for l in done_lines[-3:]:
        print("    ✓ " + re.sub(r'.*\[x\]\s*', '', l))
else:
    print("    (henüz yok)")

print("\n  Sıradaki açık tasklar:")
open_lines = [l.strip() for l in section.splitlines() if '- [ ]' in l]
if open_lines:
    for l in open_lines[:5]:
        print("    ○ " + re.sub(r'.*\[ \]\s*', '', l))
else:
    print("    (hepsi tamam!)")
PYEOF
else
  echo "  Roadmap bulunamadı: $ROADMAP"
fi

# --- 3. GIT DURUM ---
echo ""
echo "[ GİT DURUM ]"
git status --short 2>/dev/null || echo "(git status çalışmadı)"

# --- 4. TEST DURUMU ---
echo ""
echo "[ TEST DURUMU ]"

if [ -f "backend/Cargo.toml" ]; then
  echo -n "  Backend: "
  if cargo test --manifest-path backend/Cargo.toml --quiet 2>/dev/null; then
    echo "✓ geçti"
  else
    echo "✗ BAŞARISIZ — çalışmadan önce düzelt"
  fi
else
  echo "  Backend: henüz yok"
fi

if [ -f "frontend/package.json" ]; then
  echo -n "  Frontend: "
  if npm run test --prefix frontend --silent 2>/dev/null; then
    echo "✓ geçti"
  else
    echo "✗ BAŞARISIZ — çalışmadan önce düzelt"
  fi
else
  echo "  Frontend: henüz yok"
fi

if [ -f "sdk/Cargo.toml" ]; then
  echo -n "  SDK: "
  if cargo test --manifest-path sdk/Cargo.toml --quiet 2>/dev/null; then
    echo "✓ geçti"
  else
    echo "✗ BAŞARISIZ — çalışmadan önce düzelt"
  fi
else
  echo "  SDK: henüz yok"
fi

echo ""
echo "========================================="
echo "  Hazır. Nereden devam edelim?"
echo "========================================="
