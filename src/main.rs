// ============================================================
// Solana Recoverable SOL Calculator
// ============================================================
// This program scans all SPL token accounts owned by a given
// wallet and identifies "empty" ones (zero token balance).
// Empty accounts still hold a rent deposit (in lamports) that
// can be reclaimed by closing them — this tool tells you how
// much SOL you could recover.
// ============================================================

use solana_client::rpc_client::RpcClient;
use solana_client::rpc_request::TokenAccountsFilter;
use solana_sdk::pubkey::Pubkey;
use spl_token::state::Account as TokenAccount;
use solana_sdk::program_pack::Pack; // Needed for TokenAccount::unpack
use std::str::FromStr;

// 1 SOL = 1,000,000,000 lamports (the smallest unit on Solana)
const LAMPORTS_PER_SOL: f64 = 1_000_000_000.0;

fn main() {
    // --------------------------------------------------------
    // Step 1: Define the wallet public key to inspect.
    // Replace this string with any valid Solana wallet address.
    // --------------------------------------------------------
    let wallet_address = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";

    println!("============================================");
    println!(" Solana Recoverable SOL Calculator");
    println!("============================================");
    println!("Wallet: {}\n", wallet_address);

    // --------------------------------------------------------
    // Step 2: Parse the wallet address string into a Pubkey.
    // Pubkey is Solana's public key type (32-byte address).
    // --------------------------------------------------------
    let wallet_pubkey = Pubkey::from_str(wallet_address)
        .expect("Invalid wallet public key. Please provide a valid base58 Solana address.");

    // --------------------------------------------------------
    // Step 3: Connect to Solana mainnet-beta via RpcClient.
    // RpcClient talks to a Solana JSON-RPC node over HTTPS.
    // No private key or signing is required — read-only!
    // --------------------------------------------------------
    let rpc_url = "https://api.mainnet-beta.solana.com";
    let client = RpcClient::new(rpc_url.to_string());

    println!("Connected to: {}", rpc_url);
    println!("Fetching SPL token accounts...\n");

    // --------------------------------------------------------
    // Step 4: Fetch all SPL token accounts owned by the wallet.
    // We filter by the SPL Token program ID so we only get
    // token accounts, not other account types.
    // --------------------------------------------------------
    let token_program_id = spl_token::id(); // The official SPL Token program address

    let token_accounts = client
        .get_token_accounts_by_owner(
            &wallet_pubkey,
            TokenAccountsFilter::ProgramId(token_program_id),
        )
        .expect("Failed to fetch token accounts. Check your internet connection or RPC endpoint.");

    let total_accounts = token_accounts.len();
    println!("Total token accounts found: {}", total_accounts);

    // --------------------------------------------------------
    // Step 5: Loop through each account and check balance.
    // We unpack the raw account data bytes into a TokenAccount
    // struct to read the `amount` (token balance) field.
    // --------------------------------------------------------
    let mut empty_accounts: Vec<String> = Vec::new();
    let mut total_recoverable_lamports: u64 = 0;

    for keyed_account in &token_accounts {
        let address = &keyed_account.pubkey; // The token account's public key (as a string)

        // The account data is returned as base64-encoded bytes.
        // We decode and unpack it into the TokenAccount struct.
        let account_data = &keyed_account.account.data;

        // solana_account_decoder returns data as UiAccountData enum;
        // we use the parsed UI form instead — amount is in `token_amount`
        // via UiTokenAccount. We access it from the JSON-parsed form:
        if let solana_account_decoder::UiAccountData::Json(parsed) = account_data {
            // Navigate the parsed JSON to find the token balance
            if let Some(info) = parsed.parsed.get("info") {
                if let Some(token_amount) = info.get("tokenAmount") {
                    if let Some(amount_str) = token_amount.get("amount") {
                        // `amount` is a string like "0" or "1000000"
                        let amount: u64 = amount_str
                            .as_str()
                            .unwrap_or("1") // default non-zero to skip
                            .parse()
                            .unwrap_or(1);

                        if amount == 0 {
                            // This account has zero tokens — it's empty!
                            // Retrieve how many lamports (rent) it holds.
                            let lamports = keyed_account.account.lamports;
                            total_recoverable_lamports += lamports;
                            empty_accounts.push(address.clone());
                        }
                    }
                }
            }
        }
    }

    // --------------------------------------------------------
    // Step 6: Convert total lamports to SOL for display.
    // --------------------------------------------------------
    let recoverable_sol = total_recoverable_lamports as f64 / LAMPORTS_PER_SOL;

    // --------------------------------------------------------
    // Step 7: Print the results summary.
    // --------------------------------------------------------
    println!("\n============================================");
    println!(" Results");
    println!("============================================");
    println!("Total token accounts:   {}", total_accounts);
    println!("Empty accounts:         {}", empty_accounts.len());
    println!(
        "Recoverable SOL:        {:.9} SOL  ({} lamports)",
        recoverable_sol, total_recoverable_lamports
    );

    // Optional: list each empty account address
    if !empty_accounts.is_empty() {
        println!("\nEmpty account addresses:");
        for (i, addr) in empty_accounts.iter().enumerate() {
            println!("  {}. {}", i + 1, addr);
        }
    } else {
        println!("\nNo empty token accounts found for this wallet.");
    }

    println!("\n============================================");
    println!(" Tip: Close these accounts using a tool like");
    println!(" 'spl-token close' or a wallet UI (e.g.,");
    println!(" Phantom or Solflare) to reclaim the SOL.");
    println!("============================================");
}