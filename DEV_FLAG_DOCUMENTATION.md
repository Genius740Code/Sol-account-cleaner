# Dev Flag Documentation Updates

## Summary of Changes

The `-D/--dev` flag has been successfully implemented and documented across all relevant documentation files.

## Files Updated

### 1. README.md
- **Simple Usage section**: Added developer mode examples
- **Advanced Usage section**: Added dev flag examples for show, reclaim, and batch commands
- **Examples section**: Added dev flag usage examples

### 2. docs/getting-started.md
- **Single Wallet Scan options**: Added `-D, --dev` flag description
- **Batch Processing options**: Added `-D, --dev` flag description  
- **Understanding Output section**: Added comparison between normal and developer modes with note about when details are shown

### 3. docs/api.md
- **Single Wallet Scan endpoint**: Added `dev_mode` parameter documentation
- **Request Body example**: Updated to include `dev_mode` parameter

## Documentation Content

### What the Dev Flag Does

**Normal Mode (Default)**:
- Hides wallet addresses for privacy
- Hides empty account addresses 
- Shows only summary information (total accounts, empty accounts, recoverable SOL, scan time)

**Developer Mode (`-D` or `--dev`)**:
- Shows wallet addresses
- Shows all empty account addresses
- Provides detailed breakdown information in batch operations
- Shows wallet breakdown in show command
- Shows reclaim breakdown details

### Usage Examples

```bash
# Single wallet scan
solana-recover --wallet <ADDRESS> --dev

# Show command with details
solana-recover show --targets "wallet:addr1,addr2" --dev

# Reclaim with details  
solana-recover reclaim --targets "wallet:addr1,addr2" --destination <DEST> --dev

# Batch processing with details
solana-recover batch wallets.txt --dev
```

### API Integration

```json
{
  "wallet_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
  "fee_percentage": 0.15,
  "include_empty_addresses": true,
  "dev_mode": true,
  "timeout_seconds": 30
}
```

## Benefits

1. **Privacy by Default**: Sensitive information is hidden unless explicitly requested
2. **Developer Access**: Detailed information available when needed for debugging/analysis
3. **Consistent Behavior**: Same flag across all CLI commands and API endpoints
4. **Backward Compatibility**: Existing workflows continue to work unchanged

## Testing Verification

The implementation has been tested and verified:
- ✅ `-D` short flag works
- ✅ `--dev` long flag works  
- ✅ Hides information by default
- ✅ Shows details when flag is used
- ✅ Works across all subcommands
- ✅ API parameter functions correctly
