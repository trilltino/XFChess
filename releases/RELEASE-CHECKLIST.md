# XFChess Release Checklist

## 🚀 Pre-Release Checklist

### ✅ Code Quality
- [ ] Code compiles without warnings on all target platforms
- [ ] All tests pass
- [ ] Documentation is up to date
- [ ] Version numbers are updated (Cargo.toml, package.json, etc.)

### ✅ Build Verification
- [ ] Windows executable builds and runs
- [ ] Linux executable builds and runs
- [ ] macOS executable builds and runs
- [ ] Docker image builds successfully
- [ ] All assets are included in builds

### ✅ Testing
- [ ] Single player mode works
- [ ] Multiplayer host/join functionality works
- [ ] Session save/load works
- [ ] Debug mode functions correctly
- [ ] Network connectivity verified
- [ ] Graphics rendering works on all platforms

## 📦 Release Assets

### Native Executables
- [ ] `XFChess-Iroh.exe` (Windows x64)
- [ ] `xfchess` (Linux x64)
- [ ] `xfchess` (macOS x64)
- [ ] `xfchess` (macOS ARM64)

### Docker Images
- [ ] `trilltino/xfchess-iroh:latest` (multi-arch)
- [ ] `trilltino/xfchess-iroh:vX.Y.Z` (versioned)

### Documentation
- [ ] README.md with installation instructions
- [ ] Docker setup guide
- [ ] Platform-specific requirements
- [ ] Troubleshooting guide

### Launchers
- [ ] `Start-XFChess.bat` (Windows)
- [ ] `start-xfchess.sh` (Linux/macOS)
- [ ] `docker-run.sh` (Unix)
- [ ] `docker-run.bat` (Windows)

## 🐳 Docker Verification

### Build Test
```bash
docker build -t xfchess-iroh:test -f releases/Dockerfile .
```

### Runtime Test
```bash
docker run -it --rm -p 5001:5001 xfchess-iroh:test --help
docker run -it --rm -p 5001:5001 xfchess-iroh:test play --player-color white
```

### Multi-Arch Build
```bash
docker buildx build --platform linux/amd64,linux/arm64 -t trilltino/xfchess-iroh:test .
```

## 🔄 GitHub Actions

### Workflow Setup
- [ ] `.github/workflows/deploy.yml` exists and is configured
- [ ] Docker Hub credentials are set as secrets
- [ ] GITHUB_TOKEN has proper permissions
- [ ] Workflow triggers are correctly set

### Test Workflow
- [ ] Manual workflow dispatch works
- [ ] Tag-based triggers work
- [ ] All matrix builds complete successfully
- [ ] Artifacts are uploaded correctly
- [ ] Release is created with proper assets

## 🌐 Distribution

### GitHub Release
- [ ] Release notes are written
- [ ] All platform binaries are attached
- [ ] Checksums are provided
- [ ] Documentation is linked

### Docker Hub
- [ ] Image is pushed to Docker Hub
- - [ ] Multi-arch manifest is created
- [ ] Tags are properly set
- [ ] Description is updated

### Website Updates
- [ ] Download page is updated
- [ ] Docker instructions are added
- [ ] Platform compatibility matrix is updated
- [ ] Links to GitHub release are added

## 🧪 Post-Release Testing

### User Verification
- [ ] Windows users can download and run
- [ ] Linux users can run via Docker
- [ ] macOS users can run via Docker
- [ ] Multiplayer works between different platforms
- [ ] Documentation is clear and helpful

### Monitoring
- [ ] Download counts are tracked
- [ ] Issues are monitored and addressed
- [ ] User feedback is collected
- [ ] Crash reports are reviewed

## 📋 Release Template

### Tag Format
```
vX.Y.Z
```

### Release Notes Template
```markdown
## XFChess vX.Y.Z

### 🎮 New Features
- Feature 1
- Feature 2

### 🐛 Bug Fixes
- Fix 1
- Fix 2

### 🐳 Docker Support
- Multi-platform Docker images
- Cross-platform compatibility
- Easy deployment options

### 📦 Downloads
- **Windows**: `XFChess-Iroh.exe`
- **Linux**: Docker image `trilltino/xfchess-iroh`
- **macOS**: Docker image `trilltino/xfchess-iroh`

### 🚀 Quick Start
```bash
# Docker (recommended for cross-platform)
docker run -it --rm -p 5001:5001 trilltino/xfchess-iroh:latest

# Windows native
./XFChess-Iroh.exe play --player-color white
```

### 🔧 System Requirements
- Windows 10/11 (native)
- Docker Desktop (cross-platform)
- Network connection for multiplayer
- Graphics card with OpenGL 3.3+ support

### 📚 Documentation
- [Installation Guide](https://github.com/trilltino/XFChess/blob/main/releases/README.md)
- [Docker Setup](https://github.com/trilltino/XFChess/blob/main/releases/README.md#docker-setup)
- [Troubleshooting](https://github.com/trilltino/XFChess/blob/main/releases/README.md#troubleshooting)
```

## 🎯 Success Metrics

### Release Success Criteria
- [ ] All builds complete without errors
- [ ] All platforms are supported
- [ ] Docker images work correctly
- [ ] Documentation is comprehensive
- [ ] User feedback is positive

### Post-Launch Monitoring
- [ ] Download velocity
- [ ] Issue report volume
- [ ] User engagement metrics
- [ ] Platform-specific feedback

---

**Remember**: This checklist should be completed for every release to ensure quality and consistency across all platforms.
