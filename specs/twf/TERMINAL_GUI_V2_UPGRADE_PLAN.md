# Terminal.Gui v2.0 Upgrade Plan

## Overview
This document outlines the plan to upgrade TWF from Terminal.Gui v1.19.0 to v2.0. Terminal.Gui v2 represents a fundamental architectural redesign with breaking changes, but offers significant improvements in performance, testability, and modern .NET practices.

**Current Version**: Terminal.Gui 1.19.0  
**Target Version**: Terminal.Gui 2.x (latest stable)  
**Estimated Effort**: Medium-High (2-4 weeks)  
**Risk Level**: Medium

## Key Benefits of Upgrading

### Performance Improvements
- Reduced rendering overhead and event handling latency
- Snappier UI with measurably better performance
- Lower CPU usage through event-driven architecture

### Architectural Improvements
- Instance-based application model (eliminates global state)
- Full IDisposable pattern for proper resource management
- Better testability with views decoupled from global state
- Multiple application contexts support

### Enhanced Features
- 24-bit TrueColor support
- Enhanced borders and adornments system
- User-configurable themes
- Built-in scrolling for all views (no ScrollView wrapper needed)
- Advanced layout features (Dim.Auto, Pos.AnchorEnd, Pos.Align)
- View arrangement (movable/resizable windows)
- Improved keyboard and mouse handling

## Breaking Changes Analysis

### 1. Application Initialization Pattern

**v1 Pattern (Current)**:
```csharp
Application.Init();
Application.Top.Add(window);
Application.Run();
Application.Shutdown();
```

**v2 Pattern (Required)**:
```csharp
// Instance-based with proper disposal
using (IApplication app = Application.Create().Init())
{
    Window window = new() { Title = "My App" };
    app.Run(window);
    window.Dispose();
} // app.Dispose() called automatically
```

**Impact**: HIGH - Affects MainController.Initialize() and Run() methods

### 2. Dialog Execution Pattern

**v1 Pattern (Current)**:
```csharp
var dialog = new MyDialog();
Application.Run(dialog);
// Access dialog properties after
```

**v2 Pattern (Required)**:
```csharp
// For dialogs with results, use IRunnable<TResult>
app.Run<MyDialog>();
var result = app.GetResult<MyResultType>();

// Or pass instance (caller owns disposal)
var dialog = new MyDialog();
app.Run(dialog);
dialog.Dispose();
```

**Impact**: HIGH - Affects ~40+ dialog invocations throughout the codebase

### 3. Global State Access

**v1 Pattern (Current)**:
```csharp
Application.Top
Application.MainLoop.Invoke(...)
Application.Driver
Application.RequestStop()
```

**v2 Pattern (Required)**:
```csharp
// Access via View.App property
App?.Driver.Move(0, 0);
App?.TopRunnableView?.SetNeedsDraw();
App?.MainLoop.Invoke(...);
Application.RequestStop(); // Still available as static
```

**Impact**: MEDIUM - Affects ConfigurationProvider, KeyBindingManager, and MainController

### 4. Color System Changes

**v1**: Used Color.Brown  
**v2**: Uses Color.Yellow (ANSI-compliant)

**Impact**: LOW - May need to adjust color configurations

### 5. Event Handler Patterns

**v1**: Various event patterns  
**v2**: Standard EventHandler<T> pattern

**Impact**: MEDIUM - May need to update custom event handlers

### 6. Key Handling

**v1**: KeyEvent with various properties  
**v2**: Enhanced Key class with fluent API

```csharp
// v2 pattern
if (key == Key.C.WithCtrl) { }
if (key.Shift) { }
```

**Impact**: MEDIUM - KeyBindingManager may need updates

## Migration Strategy

### Phase 1: Preparation (Week 1)
1. **Create feature branch**: `feature/terminal-gui-v2-upgrade`
2. **Backup current state**: Tag current version as `pre-v2-upgrade`
3. **Review all Terminal.Gui usage**: Complete audit of Application.* calls
4. **Update dependencies**: 
   - Update Terminal.Gui package to latest v2.x
   - Verify all other dependencies are compatible
5. **Set up test environment**: Ensure all tests can run

### Phase 2: Core Application Migration (Week 2)
1. **Update MainController.Initialize()**:
   - Replace `Application.Init()` with instance-based pattern
   - Store IApplication instance as field
   - Implement proper disposal

2. **Update MainController.Run()**:
   - Change from `Application.Run()` to `app.Run(window)`
   - Update window lifecycle management

3. **Update MainController.Cleanup()**:
   - Replace `Application.Shutdown()` with `app.Dispose()`
   - Ensure proper resource cleanup

4. **Update Program.cs**:
   - Wrap controller execution in using statement
   - Handle IApplication lifecycle

### Phase 3: Dialog Migration (Week 2-3)
1. **Identify dialog patterns**:
   - Simple dialogs (no return value)
   - Dialogs with results (need IRunnable<T>)
   - Progress dialogs (special handling)

2. **Create dialog base classes** (if needed):
   - `DialogBase` for simple dialogs
   - `DialogBase<TResult>` for result-returning dialogs

3. **Migrate dialogs systematically**:
   - FileCollisionDialog
   - ConfirmationDialog
   - MessageDialog
   - CustomFunctionDialog
   - RegisterFolderDialog
   - All other dialogs (~30+ total)

4. **Update dialog invocation sites**:
   - Pass app instance or use View.App
   - Handle disposal properly
   - Extract results using new pattern

### Phase 4: Global State Elimination (Week 3)
1. **Update ConfigurationProvider**:
   - Replace `Application.Top` checks with app instance
   - Update `Application.MainLoop.Invoke` calls

2. **Update KeyBindingManager**:
   - Replace `Application.Top` checks
   - Update error dialog invocations

3. **Update MacroExpander**:
   - Update dialog execution pattern

4. **Update CustomFunctionManager**:
   - Update MenuDialog execution

### Phase 5: View Updates (Week 3-4)
1. **Review custom views**:
   - PaneView
   - TabBarView
   - TaskStatusView
   - TextViewerWindow
   - ImageViewerWindow
   - HelpView
   - All dialog classes

2. **Update view lifecycle**:
   - Implement IDisposable where needed
   - Update event handlers to EventHandler<T>
   - Use View.App property for application access

3. **Leverage new features** (optional enhancements):
   - Use built-in scrolling instead of ScrollView
   - Apply Dim.Auto for auto-sizing
   - Use Pos.AnchorEnd for bottom-aligned elements
   - Consider ViewArrangement for resizable windows

### Phase 6: Testing & Validation (Week 4)
1. **Unit tests**:
   - Update all property-based tests
   - Fix any test failures
   - Add tests for new patterns

2. **Integration testing**:
   - Test all major workflows
   - Test all dialogs
   - Test file operations
   - Test viewer windows
   - Test custom functions and menus

3. **Performance testing**:
   - Measure CPU usage (should be lower)
   - Measure UI responsiveness
   - Compare with v1 baseline

4. **Manual testing**:
   - Full application walkthrough
   - Test edge cases
   - Test on different terminals

## Code Changes Checklist

### MainController.cs
- [ ] Add `private IApplication? _app;` field
- [ ] Update `Initialize()` to use `Application.Create().Init()`
- [ ] Update `Run()` to use `_app.Run(window)`
- [ ] Update `Cleanup()` to use `_app?.Dispose()`
- [ ] Replace all `Application.Top` with `_app?.TopRunnableView`
- [ ] Replace all `Application.Driver` with `_app?.Driver`
- [ ] Update all dialog invocations (~40+ calls)

### Program.cs
- [ ] Wrap controller execution in try-finally
- [ ] Ensure proper disposal on exit

### ConfigurationProvider.cs
- [ ] Replace `Application.Top != null` checks
- [ ] Update `Application.MainLoop.Invoke` calls
- [ ] Pass app instance or use alternative pattern

### KeyBindingManager.cs
- [ ] Replace `Application.Top != null` checks
- [ ] Update `Application.MainLoop.Invoke` calls
- [ ] Update error dialog pattern

### All Dialog Classes (~30+ files)
- [ ] Consider implementing `IRunnable<TResult>` for result-returning dialogs
- [ ] Update disposal pattern
- [ ] Update event handlers if needed

### Custom Views
- [ ] PaneView: Update if using Application.* directly
- [ ] TabBarView: Update if using Application.* directly
- [ ] TaskStatusView: Update if using Application.* directly
- [ ] TextViewerWindow: Update dialog pattern
- [ ] ImageViewerWindow: Update dialog pattern
- [ ] HelpView: Update dialog pattern

### Services
- [ ] MacroExpander: Update dialog execution
- [ ] CustomFunctionManager: Update MenuDialog execution
- [ ] ViewerManager: Update if using Application.* directly

## Risk Mitigation

### Rollback Plan
1. Keep v1 branch available: `main-v1-stable`
2. Tag releases: `v1.x.x` vs `v2.x.x`
3. Document rollback procedure
4. Keep v1 compatible builds available

### Incremental Approach
1. Migrate in phases (not all at once)
2. Test after each phase
3. Commit frequently with clear messages
4. Use feature flags if needed

### Testing Strategy
1. Maintain comprehensive test suite
2. Add integration tests for critical paths
3. Manual testing checklist
4. Beta testing period before release

## Optional Enhancements (Post-Migration)

Once the core migration is complete, consider these v2-specific enhancements:

1. **Theme Support**: Implement user-configurable themes using ConfigurationManager
2. **TrueColor**: Leverage 24-bit color support for better visuals
3. **View Arrangement**: Make windows movable/resizable with ViewArrangement
4. **Auto Layout**: Use Dim.Auto for better auto-sizing
5. **Enhanced Borders**: Use new border and adornment system
6. **Gradients**: Add gradient backgrounds where appropriate
7. **Metrics**: Integrate with Terminal.Gui metrics for performance monitoring

## Timeline

| Phase | Duration | Deliverable |
|-------|----------|-------------|
| Phase 1: Preparation | 3-5 days | Branch created, dependencies updated, audit complete |
| Phase 2: Core Migration | 3-5 days | MainController and Program.cs migrated |
| Phase 3: Dialog Migration | 5-7 days | All dialogs migrated and tested |
| Phase 4: Global State | 2-3 days | All global state references eliminated |
| Phase 5: View Updates | 3-5 days | All views updated and enhanced |
| Phase 6: Testing | 3-5 days | All tests passing, manual testing complete |
| **Total** | **2-4 weeks** | **v2.0 release ready** |

## Success Criteria

1. ✅ Application builds without errors
2. ✅ All unit tests pass
3. ✅ All property-based tests pass
4. ✅ All dialogs work correctly
5. ✅ All file operations work correctly
6. ✅ All viewer windows work correctly
7. ✅ CPU usage is lower or equal to v1
8. ✅ UI feels responsive and snappy
9. ✅ No memory leaks or resource issues
10. ✅ Manual testing checklist complete

## References

- [Terminal.Gui v2 - What's New](https://gui-cs.github.io/Terminal.Gui/docs/newinv2)
- [Application Architecture Deep Dive](https://gui-cs.github.io/Terminal.Gui/docs/application.html)
- [Terminal.Gui v2 Documentation](https://gui-cs.github.io/Terminal.Gui/)
- [Terminal.Gui GitHub Repository](https://github.com/gui-cs/Terminal.Gui)

## Notes

- The migration is significant but manageable with a systematic approach
- v2 offers substantial benefits that justify the effort
- The instance-based architecture aligns better with modern .NET practices
- Proper disposal and resource management will improve application stability
- Performance improvements will enhance user experience

---

**Status**: Planning Complete  
**Next Step**: Begin Phase 1 - Preparation  
**Owner**: Development Team  
**Last Updated**: 2026-01-15
