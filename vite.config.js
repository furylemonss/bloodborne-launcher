import { defineConfig } from "vite";

// PkgExtractor game/ altındaki dosyaları kilitler; Windows'ta fs.watch → EBUSY
// beforeDevCommand'ı (vite) öldürüyor. Kurulum artıklarını asla izleme.
const installNoise = [
  "**/src-tauri/**",
  "**/game/**",
  "**/downloads/**",
  "**/port/**",
  "**/tools/**",
  "**/node_modules/**",
  "**/dist/**",
  "**/extract.log",
  "**/installer-state.json",
  "**/installer-state.json.tmp",
  "**/*.{pkg,part,zip,7z,rar,dcx,pdb,iso,img,chrbnd}",
];

export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: installNoise,
      // locked dosyada chokidar takılırsa process'i öldürme
      awaitWriteFinish: {
        stabilityThreshold: 200,
        pollInterval: 50,
      },
    },
  },
  build: {
    target: "es2021",
    minify: "esbuild",
    sourcemap: false,
  },
  plugins: [
    {
      name: "bb-ignore-watch-ebusy",
      apply: "serve",
      configureServer() {
        const watchSyscalls = new Set(["watch", "watchFile"]);
        process.on("uncaughtException", (err) => {
          const code = err && err.code;
          const syscall = err && err.syscall;
          if ((code === "EBUSY" || code === "EPERM" || code === "ENOENT") && watchSyscalls.has(syscall)) {
            return;
          }
          throw err;
        });
      },
    },
  ],
});
