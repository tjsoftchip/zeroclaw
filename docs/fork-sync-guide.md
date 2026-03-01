# Fork同步指南 - 如何保持与上游ZeroClaw同步

## 初始设置

### 1. Fork仓库

1. 访问 https://github.com/zeroclaw-labs/zeroclaw
2. 点击右上角 "Fork" 按钮
3. 选择你的账户作为目标

### 2. 克隆你的Fork

```bash
# 克隆你的fork
git clone https://github.com/YOUR_USERNAME/zeroclaw.git
cd zeroclaw

# 添加上游仓库
git remote add upstream https://github.com/zeroclaw-labs/zeroclaw.git

# 验证远程仓库
git remote -v
# 输出:
# origin    https://github.com/YOUR_USERNAME/zeroclaw.git (fetch)
# origin    https://github.com/YOUR_USERNAME/zeroclaw.git (push)
# upstream  https://github.com/zeroclaw-labs/zeroclaw.git (fetch)
# upstream  https://github.com/zeroclaw-labs/zeroclaw.git (push)
```

## 日常同步流程

### 方法1: Rebase方式（推荐，保持提交历史干净）

```bash
# 1. 获取上游更新
git fetch upstream

# 2. 切换到主分支
git checkout main

# 3. Rebase到上游
git rebase upstream/main

# 4. 推送到你的fork
git push origin main --force-with-lease
```

### 方法2: Merge方式（保留合并历史）

```bash
# 1. 获取上游更新
git fetch upstream

# 2. 切换到主分支
git checkout main

# 3. 合并上游更新
git merge upstream/main

# 4. 推送到你的fork
git push origin main
```

## 功能分支开发流程

### 创建功能分支

```bash
# 从main创建功能分支
git checkout -b feature/openwrt-smart-gateway main

# 开发并提交
git add .
git commit -m "feat: add OpenWrt smart gateway support"

# 推送到你的fork
git push origin feature/openwrt-smart-gateway
```

### 同步功能分支与上游

```bash
# 1. 获取上游更新
git fetch upstream

# 2. 切换到功能分支
git checkout feature/openwrt-smart-gateway

# 3. Rebase到上游main
git rebase upstream/main

# 4. 解决冲突（如果有）
# 编辑冲突文件，然后:
git add .
git rebase --continue

# 5. 推送（可能需要强制推送）
git push origin feature/openwrt-smart-gateway --force-with-lease
```

## 冲突解决策略

### 常见冲突场景

#### 场景1: 同一文件不同修改

```
<<<<<<< HEAD
你的修改
=======
上游修改
>>>>>>> upstream/main
```

**解决步骤**:
1. 手动编辑冲突文件，保留需要的代码
2. 运行 `git add <file>`
3. 运行 `git rebase --continue` 或 `git commit`

#### 场景2: 文件被删除/重命名

```bash
# 查看冲突状态
git status

# 如果上游删除了你修改的文件
git rm <deleted-file>
git rebase --continue

# 如果上游重命名了文件
git mv <old-name> <new-name>
git add <new-name>
git rebase --continue
```

### 冲突解决工具

```bash
# 使用VS Code作为合并工具
git config --global merge.tool vscode
git config --global mergetool.vscode.cmd 'code --wait $MERGED'

# 使用图形化工具
git mergetool
```

## 最佳实践

### 1. 保持功能分支小而专注

```bash
# 好的做法: 一个功能一个分支
feature/openwrt-support
feature/parental-control
feature/network-tools

# 不好的做法: 一个大分支包含所有功能
feature/all-new-features
```

### 2. 定期同步

```bash
# 建议每天同步一次
git fetch upstream
git checkout main
git rebase upstream/main
```

### 3. 使用.gitignore排除本地文件

```gitignore
# 本地开发文件
.env.local
*.local.toml
.idea/
.vscode/
target/
```

### 4. 提交信息规范

```bash
# 遵循Conventional Commits
feat: add new feature
fix: fix a bug
docs: update documentation
refactor: code refactoring
test: add tests
chore: maintenance tasks
```

## 自动化同步脚本

创建 `scripts/sync-upstream.sh`:

```bash
#!/bin/bash
# 同步上游仓库脚本

set -e

echo "=== Syncing with upstream ==="

# 获取上游更新
echo "Fetching upstream..."
git fetch upstream

# 切换到main
echo "Checking out main..."
git checkout main

# Rebase
echo "Rebasing on upstream/main..."
git rebase upstream/main

# 推送
echo "Pushing to origin..."
git push origin main --force-with-lease

echo "=== Sync complete ==="
```

## 提交PR到上游

```bash
# 1. 确保功能分支已同步
git checkout feature/openwrt-smart-gateway
git rebase upstream/main

# 2. 推送到你的fork
git push origin feature/openwrt-smart-gateway

# 3. 在GitHub上创建Pull Request
# 访问 https://github.com/YOUR_USERNAME/zeroclaw
# 点击 "Compare & pull request"
```

## 紧急情况处理

### 重置到上游状态

```bash
# 警告: 这会丢失你的本地修改!
git fetch upstream
git checkout main
git reset --hard upstream/main
git push origin main --force
```

### 保存当前工作

```bash
# 临时保存修改
git stash

# 同步后恢复
git stash pop
```

## 多人协作

如果你和其他人一起在你的fork上开发:

```bash
# 添加协作者的fork
git remote add collaborator https://github.com/COLLABORATOR/zeroclaw.git

# 获取协作者的更新
git fetch collaborator

# 合并协作者的分支
git merge collaborator/feature-branch
```
