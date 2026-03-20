<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { getVersion } from "@tauri-apps/api/app";
  import { onMount } from "svelte";
  import { listen, emit } from "@tauri-apps/api/event";
  import * as pdfjsLib from "pdfjs-dist";
  import workerSrc from "pdfjs-dist/build/pdf.worker.min.mjs?url";

  pdfjsLib.GlobalWorkerOptions.workerSrc = workerSrc;

  type FileType = "zip" | "pdf";

  let filePath = $state<string | null>(null);
  let fileType = $state<FileType | null>(null);
  let pageCount = $state(0);
  let zipPages = $state<string[]>([]); // 仮想パスリスト（多重圧縮対応）
  let currentIndex = $state(0); // always even in spread mode
  let spreadMode = $state(true);
  let cache = $state(new Map<number, string>());
  let pdfDoc = $state<pdfjsLib.PDFDocumentProxy | null>(null);
  let loading = $state(false);
  let errorMsg = $state<string | null>(null);
  let isDragging = $state(false);
  let showAbout = $state(false);
  let appVersion = $state("");

  // RTL: currentIndex is the RIGHT page, currentIndex+1 is the LEFT page
  let rightIdx = $derived(currentIndex);
  let leftIdx = $derived(
    spreadMode && currentIndex + 1 < pageCount ? currentIndex + 1 : -1
  );
  let rightUrl = $derived(cache.get(rightIdx) ?? null);
  let leftUrl = $derived(leftIdx >= 0 ? (cache.get(leftIdx) ?? null) : null);

  let canNext = $derived(
    spreadMode ? currentIndex + 2 < pageCount : currentIndex + 1 < pageCount
  );
  let canPrev = $derived(currentIndex > 0);

  let wheelLock = false;

  // ダイアログでファイルを選択して開く
  async function openFile() {
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: "コミック", extensions: ["zip", "cbz", "pdf"] }],
    });
    if (!selected || typeof selected !== "string") return;
    await openFileFromPath(selected);
  }

  // パスを直接指定してファイルを開く（コマンドライン引数やダイアログ共通）
  async function openFileFromPath(path: string) {
    filePath = path;
    currentIndex = 0;
    cache = new Map();
    zipPages = [];
    pdfDoc = null;
    errorMsg = null;
    loading = true;

    try {
      if (filePath.toLowerCase().endsWith(".pdf")) {
        fileType = "pdf";
        const b64: string = await invoke("get_file_base64", { path: filePath });
        const binary = atob(b64);
        const bytes = new Uint8Array(binary.length);
        for (let i = 0; i < binary.length; i++)
          bytes[i] = binary.charCodeAt(i);
        pdfDoc = await pdfjsLib.getDocument({ data: bytes }).promise;
        pageCount = pdfDoc.numPages;
      } else {
        fileType = "zip";
        const pages: string[] = await invoke("open_zip", { path: filePath });
        zipPages = pages;
        pageCount = pages.length;
      }
      await preloadVisible();
      await restorePosition();
    } catch (e) {
      errorMsg = String(e);
    } finally {
      loading = false;
    }
  }

  // 保存済みの閲覧位置を復元する
  async function restorePosition() {
    if (!filePath) return;
    const saved: string = await invoke("load_position", { filePath });
    if (!saved) return;
    if (fileType === "zip") {
      const idx = zipPages.indexOf(saved);
      if (idx >= 0) {
        currentIndex = spreadMode && idx % 2 !== 0 ? idx - 1 : idx;
        await preloadVisible();
      }
    } else if (fileType === "pdf") {
      const idx = parseInt(saved, 10);
      if (!isNaN(idx) && idx > 0 && idx < pageCount) {
        currentIndex = spreadMode && idx % 2 !== 0 ? idx - 1 : idx;
        await preloadVisible();
      }
    }
  }

  // 現在の閲覧位置を保存する
  async function savePosition() {
    if (!filePath) return;
    const position =
      fileType === "zip"
        ? (zipPages[currentIndex] ?? "")
        : String(currentIndex);
    if (position) {
      await invoke("save_position", { filePath, position });
    }
  }

  // 対応ファイル拡張子チェック
  function isSupportedFile(path: string): boolean {
    const lower = path.toLowerCase();
    return lower.endsWith(".zip") || lower.endsWith(".cbz") || lower.endsWith(".pdf");
  }

  // 起動時にバックエンドからファイルパスが送られてきたら自動オープン
  onMount(async () => {
    appVersion = await getVersion();

    // "open-file" イベントを先に登録してから準備完了を通知する
    const unlisten = await listen<string>("open-file", async (event) => {
      unlisten();
      await openFileFromPath(event.payload);
    });
    // バックエンドに「フロントエンドの IPC 準備完了」を通知
    await emit("frontend-ready", null);

    // エクスプローラ等からのドラッグ＆ドロップ対応
    await listen("tauri://drag-enter", () => { isDragging = true; });
    await listen("tauri://drag-over",  () => { isDragging = true; });
    await listen("tauri://drag-leave", () => { isDragging = false; });
    await listen<{ paths: string[]; position: unknown }>("tauri://drag-drop", (event) => {
      isDragging = false;
      const path = event.payload?.paths?.[0];
      if (path && isSupportedFile(path)) {
        openFileFromPath(path);
      }
    });
  });

  async function loadPage(index: number): Promise<string> {
    if (cache.has(index)) return cache.get(index)!;

    let url: string;
    if (fileType === "zip") {
      const virtualPath = zipPages[index];
      const result: { data: string; mime: string } = await invoke(
        "get_zip_page",
        { path: filePath, virtualPath }
      );
      url = `data:${result.mime};base64,${result.data}`;
    } else if (fileType === "pdf" && pdfDoc) {
      const page = await pdfDoc.getPage(index + 1); // PDF.js は 1-indexed
      const viewport = page.getViewport({ scale: 1.5 });
      const canvas = document.createElement("canvas");
      canvas.width = viewport.width;
      canvas.height = viewport.height;
      const ctx = canvas.getContext("2d")!;
      await page.render({ canvasContext: ctx, viewport }).promise;
      url = canvas.toDataURL("image/jpeg", 0.92);
    } else {
      return "";
    }

    cache = new Map(cache).set(index, url);
    return url;
  }

  async function preloadVisible() {
    const toLoad = [rightIdx];
    if (leftIdx >= 0) toLoad.push(leftIdx);
    await Promise.all(toLoad.map(loadPage));

    // 次のスプレッドを先読み
    const next1 = spreadMode ? currentIndex + 2 : currentIndex + 1;
    const next2 = spreadMode ? currentIndex + 3 : -1;
    [next1, next2]
      .filter((i) => i >= 0 && i < pageCount)
      .forEach((i) => loadPage(i));
  }

  async function goNext() {
    if (!canNext) return;
    currentIndex = spreadMode ? currentIndex + 2 : currentIndex + 1;
    await preloadVisible();
    await savePosition();
  }

  async function goPrev() {
    if (!canPrev) return;
    if (spreadMode) {
      currentIndex = Math.max(0, currentIndex - 2);
      if (currentIndex % 2 !== 0) currentIndex--;
    } else {
      currentIndex--;
    }
    await preloadVisible();
    await savePosition();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "ArrowLeft" || e.key === " ") {
      e.preventDefault();
      goNext(); // RTL: 左キー = 次へ（ページ進む）
    } else if (e.key === "ArrowRight" || e.key === "Backspace") {
      e.preventDefault();
      goPrev(); // RTL: 右キー = 前へ（ページ戻る）
    }
  }

  function handleClick(e: MouseEvent) {
    if (!filePath || loading) return;
    const target = e.currentTarget as HTMLElement;
    const rect = target.getBoundingClientRect();
    const x = e.clientX - rect.left;
    if (x < rect.width / 2) {
      goNext(); // 左半分クリック = 次へ（RTL）
    } else {
      goPrev(); // 右半分クリック = 前へ（RTL）
    }
  }

  function handleWheel(e: WheelEvent) {
    if (!filePath || loading || wheelLock) return;
    e.preventDefault();
    wheelLock = true;
    setTimeout(() => (wheelLock = false), 300);
    if (e.deltaY > 0) {
      goNext();
    } else {
      goPrev();
    }
  }

  function toggleSpread() {
    spreadMode = !spreadMode;
    if (spreadMode && currentIndex % 2 !== 0) currentIndex--;
    preloadVisible();
  }

  // ページ表示文字列
  let pageLabel = $derived(
    pageCount > 0
      ? `${currentIndex + 1}${spreadMode && leftIdx >= 0 ? `–${leftIdx + 1}` : ""} / ${pageCount}`
      : ""
  );

  // フルパスからファイル名だけを取り出す
  function basename(path: string): string {
    return path.replace(/\\/g, "/").split("/").pop() ?? path;
  }

  // 仮想パス（"::"区切り）を表示用に整形する
  // 例: "vol1.zip::ch1.zip::001.jpg" → "vol1.zip > ch1.zip > 001.jpg"
  function formatVirtualPath(vpath: string): string {
    return vpath.split("::").join(" > ");
  }

  // 右ページの表示情報
  let rightPageInfo = $derived(
    fileType === "zip" && zipPages[rightIdx]
      ? formatVirtualPath(zipPages[rightIdx])
      : fileType === "pdf"
        ? `ページ ${rightIdx + 1}`
        : ""
  );

  // 左ページの表示情報
  let leftPageInfo = $derived(
    leftIdx >= 0
      ? fileType === "zip" && zipPages[leftIdx]
        ? formatVirtualPath(zipPages[leftIdx])
        : fileType === "pdf"
          ? `ページ ${leftIdx + 1}`
          : ""
      : ""
  );

  // 外側のファイル名
  let outerFileName = $derived(filePath ? basename(filePath) : "");

  // ウィンドウタイトルを自動更新
  $effect(() => {
    if (!filePath) {
      getCurrentWindow().setTitle("Comic Viewer");
      return;
    }
    let title = `Comic Viewer — ${outerFileName}`;
    if (rightPageInfo) title += `  [${rightPageInfo}${leftPageInfo ? " | " + leftPageInfo : ""}]`;
    getCurrentWindow().setTitle(title);
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="app" class:dragging={isDragging}>
  {#if isDragging}
    <div class="drag-overlay">
      <div class="drag-message">ここにドロップして開く</div>
    </div>
  {/if}
  <div class="toolbar">
    <div class="toolbar-row">
      <button onclick={openFile}>開く</button>
      <button onclick={toggleSpread} class:active={spreadMode}>
        {spreadMode ? "見開き" : "単ページ"}
      </button>
      {#if pageCount > 0}
        <span class="page-info">{pageLabel}</span>
      {/if}
      <span class="nav-hint">← 次へ　→ 前へ</span>
      <button class="about-btn" onclick={() => showAbout = true}>このソフトについて</button>
    </div>
    {#if filePath && pageCount > 0}
      <div class="file-info-row">
        <span class="outer-file">{outerFileName}</span>
        {#if spreadMode && leftPageInfo}
          <span class="page-path"><span class="page-side right-side">右</span>{rightPageInfo}</span>
          <span class="path-sep">/</span>
          <span class="page-path"><span class="page-side left-side">左</span>{leftPageInfo}</span>
        {:else if rightPageInfo}
          <span class="page-path">{rightPageInfo}</span>
        {/if}
      </div>
    {/if}
  </div>

  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="viewer"
    onclick={handleClick}
    onwheel={handleWheel}
    role="button"
    tabindex="0"
  >
    {#if loading}
      <div class="status">読み込み中...</div>
    {:else if errorMsg}
      <div class="status error">{errorMsg}</div>
    {:else if !filePath}
      <div class="status welcome">
        <p class="welcome-title">Comic Viewer</p>
        <p class="welcome-sub">対応形式: ZIP / CBZ / PDF</p>
        <button onclick={openFile}>ファイルを開く</button>
      </div>
    {:else if spreadMode}
      <div class="spread">
        <!-- RTL: 左側 = 後のページ、右側 = 現在ページ -->
        <div class="page-slot">
          {#if leftUrl}
            <img src={leftUrl} alt="左ページ" draggable="false" />
          {:else if leftIdx >= 0}
            <div class="page-placeholder">…</div>
          {/if}
        </div>
        <div class="page-slot">
          {#if rightUrl}
            <img src={rightUrl} alt="右ページ" draggable="false" />
          {:else}
            <div class="page-placeholder">…</div>
          {/if}
        </div>
      </div>
    {:else}
      <div class="single">
        {#if rightUrl}
          <img src={rightUrl} alt="ページ" draggable="false" />
        {:else}
          <div class="page-placeholder">…</div>
        {/if}
      </div>
    {/if}
  </div>
</div>

{#if showAbout}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="modal-overlay" onclick={() => showAbout = false}>
    <div class="modal" onclick={(e) => e.stopPropagation()}>
      <h2>Comic Viewer</h2>
      <p>バージョン: {appVersion}</p>
      <button onclick={() => showAbout = false}>閉じる</button>
    </div>
  </div>
{/if}

<style>
  :global(body),
  :global(html) {
    margin: 0;
    padding: 0;
    background: #111;
    color: #ddd;
    font-family: system-ui, "Segoe UI", sans-serif;
    overflow: hidden;
    height: 100vh;
  }

  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }

  .toolbar {
    display: flex;
    flex-direction: column;
    padding: 4px 10px;
    background: #222;
    border-bottom: 1px solid #333;
    flex-shrink: 0;
  }

  .toolbar-row {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 32px;
  }

  .file-info-row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 2px 0 4px;
    font-size: 12px;
    color: #888;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .outer-file {
    color: #aaa;
    font-weight: 600;
    flex-shrink: 0;
  }

  .outer-file::after {
    content: "  |";
    color: #555;
    margin-left: 4px;
  }

  .page-path {
    display: flex;
    align-items: center;
    gap: 4px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: #888;
  }

  .page-side {
    font-size: 10px;
    padding: 1px 4px;
    border-radius: 3px;
    flex-shrink: 0;
  }

  .right-side {
    background: #3a3a5a;
    color: #99aaff;
  }

  .left-side {
    background: #3a4a3a;
    color: #99cc99;
  }

  .path-sep {
    color: #555;
    flex-shrink: 0;
  }

  .toolbar button {
    padding: 3px 12px;
    background: #3a3a3a;
    color: #ddd;
    border: 1px solid #555;
    border-radius: 4px;
    cursor: pointer;
    font-size: 13px;
  }

  .toolbar button:hover {
    background: #4a4a4a;
  }

  .toolbar button.active {
    background: #505070;
    border-color: #777;
  }

  .page-info {
    font-size: 13px;
    color: #999;
    min-width: 80px;
  }

  .nav-hint {
    font-size: 11px;
    color: #555;
    margin-left: auto;
  }

  .viewer {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    user-select: none;
    cursor: pointer;
    outline: none;
  }

  .status {
    text-align: center;
    color: #666;
  }

  .status.error {
    color: #f66;
    max-width: 500px;
    word-break: break-all;
  }

  .welcome {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
  }

  .welcome-title {
    font-size: 28px;
    color: #aaa;
    margin: 0;
  }

  .welcome-sub {
    font-size: 14px;
    color: #555;
    margin: 0;
  }

  .welcome button {
    margin-top: 8px;
    padding: 10px 28px;
    background: #3a3a3a;
    color: #ddd;
    border: 1px solid #555;
    border-radius: 6px;
    cursor: pointer;
    font-size: 15px;
  }

  .welcome button:hover {
    background: #4a4a4a;
  }

  .spread {
    display: flex;
    height: 100%;
    width: 100%;
    align-items: center;
    justify-content: center;
    gap: 1px;
  }

  .page-slot {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    max-width: 50%;
    flex: 1;
  }

  .page-slot img {
    max-height: 100%;
    max-width: 100%;
    object-fit: contain;
    display: block;
  }

  .single {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    width: 100%;
  }

  .single img {
    max-height: 100%;
    max-width: 100%;
    object-fit: contain;
  }

  .page-placeholder {
    width: 300px;
    height: 420px;
    background: #222;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #444;
    font-size: 24px;
  }

  /* ドラッグ＆ドロップ */
  .drag-overlay {
    position: fixed;
    inset: 0;
    z-index: 100;
    background: rgba(80, 100, 180, 0.35);
    border: 3px dashed #7799ff;
    display: flex;
    align-items: center;
    justify-content: center;
    pointer-events: none;
  }

  .drag-message {
    font-size: 28px;
    color: #ccdaff;
    background: rgba(0, 0, 40, 0.7);
    padding: 20px 40px;
    border-radius: 12px;
  }

  .app.dragging .viewer {
    pointer-events: none;
  }

  .about-btn {
    margin-left: auto;
  }

  .modal-overlay {
    position: fixed;
    inset: 0;
    z-index: 200;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .modal {
    background: #2a2a2a;
    border: 1px solid #444;
    border-radius: 8px;
    padding: 32px 40px;
    text-align: center;
    min-width: 260px;
  }

  .modal h2 {
    margin: 0 0 16px;
    font-size: 20px;
    color: #eee;
  }

  .modal p {
    margin: 0 0 24px;
    color: #bbb;
  }
</style>
