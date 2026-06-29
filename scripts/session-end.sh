#!/usr/bin/env bash
# Çalışma bitiminde çalıştırılır.
# Kontrol eder: testler, commit edilmemiş task tikleri, session dosyası güncel mi.

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

ROADMAP="streamdubber_roadmap_final.md"
ISSUES=0

echo "========================================="
echo "  StreamDubber — Session Bitiş Kontrolü"
echo "========================================="

# --- 1. TEST DURUMU ---
echo ""
echo "[ TEST DURUMU ]"

if [ -f "backend/Cargo.toml" ]; then
  echo -n "  Backend: "
  if cargo test --manifest-path backend/Cargo.toml --quiet 2>/dev/null; then
    echo "✓ geçti"
  else
    echo "✗ BAŞARISIZ — commit etmeden önce düzelt"
    ISSUES=$((ISSUES + 1))
  fi
else
  echo "  Backend: henüz yok"
fi

if [ -f "frontend/package.json" ]; then
  echo -n "  Frontend: "
  if npm run test --prefix frontend --silent 2>/dev/null; then
    echo "✓ geçti"
  else
    echo "✗ BAŞARISIZ — commit etmeden önce düzelt"
    ISSUES=$((ISSUES + 1))
  fi
else
  echo "  Frontend: henüz yok"
fi

if [ -f "sdk/Cargo.toml" ]; then
  echo -n "  SDK: "
  if cargo test --manifest-path sdk/Cargo.toml --quiet 2>/dev/null; then
    echo "✓ geçti"
  else
    echo "✗ BAŞARISIZ — commit etmeden önce düzelt"
    ISSUES=$((ISSUES + 1))
  fi
else
  echo "  SDK: henüz yok"
fi

# --- 2. COMMIT EDİLMEMİŞ TASK TİKİ ---
echo ""
echo "[ ROADMAP KONTROLü ]"

if git diff "$ROADMAP" 2>/dev/null | grep -qF '+- [x]'; then
  echo "  ⚠ Tik atılmış ama commit edilmemiş task var!"
  echo "  → bash scripts/commit-task.sh \"task açıklaması\" çalıştır"
  ISSUES=$((ISSUES + 1))
else
  echo "  ✓ Tüm task tikleri commit edilmiş"
fi

# --- 3. KOMİT EDİLMEMİŞ GENEL DEĞİŞİKLİK ---
echo ""
echo "[ GİT DURUM ]"
uncommitted=$(git status --short 2>/dev/null)
if [ -n "$uncommitted" ]; then
  echo "  ⚠ Commit edilmemiş değişiklikler var:"
  echo "$uncommitted" | sed 's/^/    /'
  ISSUES=$((ISSUES + 1))
else
  echo "  ✓ Temiz — commit edilmemiş dosya yok"
fi

# --- 4. SESSION DOSYASI GÜNCEL Mİ ---
echo ""
echo "[ SESSION DOSYASI ]"

SESSION_FILE=""
if [ -f "docs/sessions/claude.md" ] && [ -f "docs/sessions/codex.md" ]; then
  # Hangi agent olduğunu tahmin et (çevre değişkenine bak, yoksa genel uyarı)
  if [ -n "${CLAUDE_SESSION_ID:-}" ] || [ -n "${ANTHROPIC_API_KEY:-}" ]; then
    SESSION_FILE="docs/sessions/claude.md"
  else
    SESSION_FILE="docs/sessions/codex.md"
  fi
fi

if [ -n "$SESSION_FILE" ] && [ -f "$SESSION_FILE" ]; then
  last_modified=$(git log -1 --format="%ar" -- "$SESSION_FILE" 2>/dev/null)
  if [ -z "$last_modified" ]; then
    echo "  ⚠ $SESSION_FILE hiç commit edilmemiş — güncelle ve commit et"
    ISSUES=$((ISSUES + 1))
  else
    echo "  ✓ $SESSION_FILE son güncelleme: $last_modified"
  fi
else
  echo "  ⚠ Kendi session dosyasını (docs/sessions/claude.md veya codex.md) güncelle"
  ISSUES=$((ISSUES + 1))
fi

# --- 5. FAZ 1 ÖZET ---
echo ""
echo "[ FAZ 1 DURUM ]"
if [ -f "$ROADMAP" ]; then
  python3 - "$ROADMAP" <<'PYEOF'
import sys, re
roadmap = open(sys.argv[1]).read()
m = re.search(r'(## Faz 1.*?)(## Faz 2)', roadmap, re.DOTALL)
if m:
    section = m.group(1)
    total = len(re.findall(r'- \[[x ]\]', section))
    done  = len(re.findall(r'- \[x\]', section))
    print(f"  {done} / {total} task tamamlandı")
PYEOF
fi

# --- SONUÇ ---
echo ""
echo "========================================="
if [ $ISSUES -eq 0 ]; then
  echo "  ✓ Her şey temiz. Session kapatabilirsin."
else
  echo "  ⚠ $ISSUES sorun var — yukarıdaki uyarıları çöz."
fi
echo "========================================="
