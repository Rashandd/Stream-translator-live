#!/usr/bin/env bash
# Kullanım: bash scripts/commit-task.sh "task açıklaması"
#
# Task tamamlandığında:
#   1. Roadmap'teki tiki commit eder
#   2. Çakışma varsa bekler, pull --rebase yapar, tekrar dener
#   3. Hâlâ çakışma varsa durumu açıklar — elle müdahale gerekir

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

ROADMAP="streamdubber_roadmap_final.md"
TASK_DESC="${1:-task tamamlandı}"
MAX_RETRY=3
RETRY_WAIT=5  # saniye

if [ ! -f "$ROADMAP" ]; then
  echo "HATA: $ROADMAP bulunamadı"
  exit 1
fi

commit_and_push() {
  git add "$ROADMAP"

  # Commit edilecek değişiklik var mı?
  if git diff --cached --quiet; then
    echo "Roadmap'te commit edilecek değişiklik yok. Önce tiki at: [ ] → [x]"
    exit 1
  fi

  git commit -m "chore: ✓ $TASK_DESC"
}

push_with_retry() {
  local attempt=1
  while [ $attempt -le $MAX_RETRY ]; do
    echo "→ Push deneniyor ($attempt/$MAX_RETRY)..."

    if git push 2>/dev/null; then
      echo "✓ Push başarılı."
      return 0
    fi

    echo "  Push başarısız — çakışma olabilir. $RETRY_WAIT saniye bekleniyor..."
    sleep $RETRY_WAIT

    echo "  Pull --rebase yapılıyor..."
    if git pull --rebase 2>/dev/null; then
      echo "  Rebase başarılı, tekrar deneniyor..."
    else
      echo ""
      echo "⚠ ÇAKIŞMA — otomatik çözülemiyor."
      echo ""
      echo "Yapılacaklar:"
      echo "  1. git status   → çakışan dosyaya bak"
      echo "  2. Roadmap'i aç → <<<< işaretlerini bul"
      echo "  3. Her iki tiki de [x] yap, çakışma işaretlerini sil"
      echo "  4. git add $ROADMAP"
      echo "  5. git rebase --continue"
      echo "  6. git push"
      exit 1
    fi

    attempt=$((attempt + 1))
  done

  echo "✗ $MAX_RETRY denemeden sonra push başarısız. Elle kontrol et."
  exit 1
}

# Remote var mı kontrol et
if git remote get-url origin &>/dev/null; then
  commit_and_push
  push_with_retry
else
  # Remote yok (local-only), sadece commit
  commit_and_push
  echo "✓ Commit edildi (remote yok, push atlandı)."
fi
