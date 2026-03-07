# Unified Proxy Implementation Summary

## Overview
The unified proxy implementation successfully merges the separate SOCKS5 and HTTP CONNECT proxy ports into a single port that can handle both protocols simultaneously. The implementation uses protocol detection based on the first byte of incoming connections to determine whether to route the request to the SOCKS5 or HTTP CONNECT handler.

## Changes Made

### 1. Core Proxy Module (`../clash-lib/src/proxy/unified_proxy.rs`)
- Created a new unified proxy module that handles both SOCKS5 and HTTP CONNECT protocols
- Implemented protocol detection logic to identify incoming connection type
- Used a custom `BytePrefixedReader` to handle the peeked byte in the processing pipeline
- Maintained all existing functionality while consolidating to a single port

### 2. Proxy Module Registration (`../clash-lib/src/proxy/mod.rs`)
- Added the unified_proxy module to the proxy module system
- Ensured proper imports and visibility

### 3. Backend Architecture (`./src-tauri/src/swarm_task.rs`)
- Modified `StartProxy` command to accept a single `unified_port` instead of separate `socks5_port` and `http_port`
- Updated proxy startup logic to use the unified proxy instead of separate SOCKS5 and HTTP proxies
- Adjusted system proxy configuration to use the unified port for both HTTP and SOCKS settings

### 4. State Management (`./src-tauri/src/state.rs`)
- Updated `ProxyStatus` struct to use `unified_port` instead of separate ports
- Modified `AppCommand::StartProxy` to accept a single `unified_port` parameter

### 5. API Commands (`./src-tauri/src/commands.rs`)
- Updated `start_proxy` command to accept a single `unified_port` parameter
- Maintained backward compatibility for API surface where possible

### 6. Configuration (`../clash-lib/src/config/mod.rs`)
- Updated `NodeConfig` to use `unified_port` instead of separate `socks5_port` and `http_port`
- Changed default unified port to 7890
- Removed obsolete port fields

### 7. System Proxy Configuration (`../clash-lib/src/sys_proxy.rs`)
- Updated system proxy setup to use the same port for both HTTP and SOCKS configurations
- Modified platform-specific implementations (macOS, Windows, Linux) to use unified port
- Added appropriate logging and status messages

### 8. Frontend UI (`./src/pages/settings.rs`)
- Updated settings page to display single unified proxy port
- Changed UI labels from "Proxy Ports" to "Proxy Port"
- Modified display to show only the unified port instead of separate SOCKS5 and HTTP ports

### 9. API Layer (`./src/api.rs`)
- Updated `start_proxy` function to accept a single `unified_port` parameter
- Modified argument serialization to match the new command structure

### 10. Type Definitions (`./src/types.rs`)
- Updated `ProxyStatus` struct to use `unified_port` instead of separate ports
- Maintained other fields unchanged

## Benefits

1. **Simplified Configuration**: Users now only need to manage a single proxy port
2. **Reduced Resource Usage**: Single TCP listener instead of two
3. **Maintained Functionality**: All existing proxy functionality is preserved
4. **Automatic Protocol Detection**: No manual configuration needed to choose protocol
5. **Consistent Experience**: Same unified port for all proxy traffic

## Technical Details

The implementation uses the following approach for protocol detection:
- SOCKS5: First byte is typically `0x05` (version indicator)
- HTTP CONNECT: First byte is an ASCII character (typically 'C' for "CONNECT")

The custom `BytePrefixedReader` ensures that the peeked byte is properly placed back into the stream for processing by the appropriate handler.

## Default Port

The default unified proxy port is set to `7890`, which is a common default for unified proxy configurations.

## Migration Path

Existing configurations using separate ports will need to be updated to use the unified port. The implementation maintains the same external interfaces where possible to minimize disruption.