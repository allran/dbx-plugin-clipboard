本地调试
```
nvm use
dbx-plugin dev --path . --port 5190
```

### 1. 打包
本插件带 Rust Sidecar，会打出**当前平台**的包：
```bash
cd /Users/alan/VsCodeProjects/alan/dbx-plugin-clipboard
nvm use          # Node 22+
npm install      # 若依赖已装可跳过
npm run build
dbx-plugin package .
```

产物在 `dist/`，例如：
- `alan.clipboard-0.1.0-darwin-arm64.dbxp`（或 `darwin-x64`）
- 同名的 `.artifact.json`

### 2. 装进 DBX（本地开发）
1. 打开 DBX → **插件中心** → **设置**
2. 展开 **第三方与开发者选项**
3. 开启 **允许安装未签名开发包**
4. 点 **安装 `.dbxp`**，选 `dist/` 里那个文件
5. 到 **已安装** 里打开「剪贴板历史」工作台

未签名开发包可用**同版本**反复重装；测完建议关掉该开关。
---

若要上官方商店：先发 GitHub Release（带 `release-candidates.json`），再往 `t8y2/dbx-store` 提候选 PR，由维护者签名上架——日常自测用上面本地安装即可。需要的话我可以直接帮你在本机跑一遍 `package`。