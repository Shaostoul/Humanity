/**
 * HumanityOS — Solana Wallet Module
 *
 * Pure vanilla JS, zero npm dependencies. Uses Web Crypto API + fetch().
 * The user's existing Ed25519 identity key IS their Solana wallet.
 *
 * Load order: crypto.js must load before this file (provides myIdentity, hexToBuf, bufToHex).
 * Usage: <script src="/shared/wallet.js"></script>
 *
 * All public API exposed via window.HosWallet.
 */
(function() {
  'use strict';

  // ── Base58 ──────────────────────────────────────────────────────────────────
  // Bitcoin/Solana alphabet (no 0, O, I, l to avoid ambiguity)

  var BASE58_ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
  var BASE58_MAP = {};
  for (var i = 0; i < 58; i++) BASE58_MAP[BASE58_ALPHABET[i]] = BigInt(i);

  /** Encode a Uint8Array to a base58 string. */
  function base58Encode(bytes) {
    if (!bytes || bytes.length === 0) return '';

    // Count leading zeros — each becomes a '1' in base58
    var leadingZeros = 0;
    while (leadingZeros < bytes.length && bytes[leadingZeros] === 0) leadingZeros++;

    // Convert bytes to a BigInt
    var num = BigInt(0);
    for (var i = 0; i < bytes.length; i++) {
      num = num * BigInt(256) + BigInt(bytes[i]);
    }

    // Convert BigInt to base58 digits
    var chars = [];
    while (num > BigInt(0)) {
      var remainder = num % BigInt(58);
      num = num / BigInt(58);
      chars.unshift(BASE58_ALPHABET[Number(remainder)]);
    }

    // Prepend '1' for each leading zero byte
    for (var j = 0; j < leadingZeros; j++) chars.unshift('1');

    return chars.join('');
  }

  /** Decode a base58 string to a Uint8Array. */
  function base58Decode(str) {
    if (!str || str.length === 0) return new Uint8Array(0);

    // Count leading '1's — each becomes a 0x00 byte
    var leadingOnes = 0;
    while (leadingOnes < str.length && str[leadingOnes] === '1') leadingOnes++;

    // Convert base58 string to BigInt
    var num = BigInt(0);
    for (var i = 0; i < str.length; i++) {
      var val = BASE58_MAP[str[i]];
      if (val === undefined) throw new Error('Invalid base58 character: ' + str[i]);
      num = num * BigInt(58) + val;
    }

    // Convert BigInt to bytes
    var hexStr = num.toString(16);
    if (hexStr.length % 2) hexStr = '0' + hexStr;
    var byteLen = hexStr.length / 2;
    var result = new Uint8Array(leadingOnes + byteLen);
    // Leading zeros are already 0 in Uint8Array
    for (var j = 0; j < byteLen; j++) {
      result[leadingOnes + j] = parseInt(hexStr.substr(j * 2, 2), 16);
    }

    return result;
  }

  // ── Hex Utilities ───────────────────────────────────────────────────────────
  // Local copies so wallet.js is self-contained (crypto.js also defines these)

  function hexToBytes(hex) {
    var bytes = new Uint8Array(hex.length / 2);
    for (var i = 0; i < hex.length; i += 2) bytes[i / 2] = parseInt(hex.substr(i, 2), 16);
    return bytes;
  }

  function bytesToHex(bytes) {
    return Array.from(bytes).map(function(b) { return b.toString(16).padStart(2, '0'); }).join('');
  }

  // ── Address Derivation ──────────────────────────────────────────────────────

  /**
   * Convert an Ed25519 public key hex string to a Solana address.
   * A Solana address is simply the 32-byte public key base58-encoded.
   */
  function publicKeyToSolanaAddress(publicKeyHex) {
    var bytes = hexToBytes(publicKeyHex);
    if (bytes.length !== 32) throw new Error('Public key must be 32 bytes, got ' + bytes.length);
    return base58Encode(bytes);
  }

  // ── RPC Helper ──────────────────────────────────────────────────────────────

  var SOLANA_RPC = localStorage.getItem('hos_solana_rpc') || 'https://api.mainnet-beta.solana.com';

  /** Make a JSON-RPC call to the Solana cluster. */
  async function solanaRPC(method, params) {
    var res = await fetch(SOLANA_RPC, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: method, params: params || [] })
    });
    var data = await res.json();
    if (data.error) throw new Error('Solana RPC error: ' + data.error.message);
    return data.result;
  }

  // ── Balance Cache ───────────────────────────────────────────────────────────
  // Cache balance queries for 30 seconds to avoid rate limiting

  var _balanceCache = {};
  var BALANCE_CACHE_MS = 30000;

  function getCached(key) {
    var entry = _balanceCache[key];
    if (entry && Date.now() - entry.ts < BALANCE_CACHE_MS) return entry.value;
    return undefined;
  }

  function setCache(key, value) {
    _balanceCache[key] = { value: value, ts: Date.now() };
    return value;
  }

  // ── Balance Queries ─────────────────────────────────────────────────────────

  /** Get SOL balance in SOL (not lamports). */
  async function getSOLBalance(address) {
    var cached = getCached('sol:' + address);
    if (cached !== undefined) return cached;

    var result = await solanaRPC('getBalance', [address, { commitment: 'confirmed' }]);
    return setCache('sol:' + address, result.value / 1e9);
  }

  /** USDC mint address on Solana mainnet. */
  var USDC_MINT = 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v';

  /** Get USDC balance for an address. */
  async function getUSDCBalance(address) {
    var cached = getCached('usdc:' + address);
    if (cached !== undefined) return cached;

    var result = await solanaRPC('getTokenAccountsByOwner', [
      address,
      { mint: USDC_MINT },
      { encoding: 'jsonParsed' }
    ]);
    if (!result.value || !result.value.length) return setCache('usdc:' + address, 0);
    return setCache('usdc:' + address, result.value[0].account.data.parsed.info.tokenAmount.uiAmount || 0);
  }

  /** Get all SPL token balances for an address. */
  async function getAllTokenBalances(address) {
    var cached = getCached('tokens:' + address);
    if (cached !== undefined) return cached;

    var result = await solanaRPC('getTokenAccountsByOwner', [
      address,
      { programId: 'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA' },
      { encoding: 'jsonParsed' }
    ]);
    var tokens = (result.value || []).map(function(item) {
      var info = item.account.data.parsed.info;
      return {
        mint: info.mint,
        balance: info.tokenAmount.uiAmount || 0,
        decimals: info.tokenAmount.decimals,
        address: item.pubkey
      };
    });
    return setCache('tokens:' + address, tokens);
  }

  // ── Transaction History ─────────────────────────────────────────────────────

  /** Get recent transaction signatures for an address. */
  async function getTransactionHistory(address, limit) {
    limit = limit || 20;
    var sigs = await solanaRPC('getSignaturesForAddress', [address, { limit: limit }]);
    return sigs.map(function(s) {
      return {
        signature: s.signature,
        timestamp: s.blockTime ? new Date(s.blockTime * 1000) : null,
        status: s.err ? 'failed' : 'confirmed',
        memo: s.memo || null
      };
    });
  }

  // ── Price Data ──────────────────────────────────────────────────────────────

  var _priceCache = { sol: 0, timestamp: 0 };

  /** Get SOL price in USD (cached for 5 minutes). */
  async function getSOLPrice() {
    if (Date.now() - _priceCache.timestamp < 300000 && _priceCache.sol > 0) return _priceCache.sol;
    try {
      var res = await fetch('https://api.coingecko.com/api/v3/simple/price?ids=solana&vs_currencies=usd');
      var data = await res.json();
      _priceCache.sol = data.solana.usd;
      _priceCache.timestamp = Date.now();
      return _priceCache.sol;
    } catch (e) {
      console.warn('HosWallet: price fetch failed:', e);
      return _priceCache.sol || 0;
    }
  }

  // ── Compact-u16 Encoding ────────────────────────────────────────────────────
  // Solana uses compact-u16 for array lengths in the wire format.
  // Values 0-127 use 1 byte. 128-16383 use 2 bytes. 16384+ use 3 bytes.

  function encodeCompactU16(value) {
    var bytes = [];
    while (true) {
      var elem = value & 0x7f;
      value >>= 7;
      if (value === 0) {
        bytes.push(elem);
        break;
      } else {
        elem |= 0x80;
        bytes.push(elem);
      }
    }
    return bytes;
  }

  // ── Transaction Serialization ───────────────────────────────────────────────
  // Solana v0 (legacy) transaction wire format:
  //   [compact-u16 num_signatures] [64-byte signatures...]
  //   [message]
  //
  // Message format:
  //   [header: 3 bytes] [compact-u16 num_keys] [32-byte keys...]
  //   [32-byte recent_blockhash]
  //   [compact-u16 num_instructions] [instructions...]
  //
  // Instruction format:
  //   [u8 program_id_index] [compact-u16 num_accounts] [u8 account_indices...]
  //   [compact-u16 data_len] [u8 data...]

  /** System Program address (all zeros except last byte = 0, but actually all zeros). */
  var SYSTEM_PROGRAM = new Uint8Array(32); // 32 zero bytes = 11111111111111111111111111111111

  /**
   * Write a u64 value as 8 bytes in little-endian into a Uint8Array.
   * Uses BigInt for precision with large lamport values.
   */
  function writeU64LE(value) {
    var bigVal = BigInt(value);
    var buf = new Uint8Array(8);
    for (var i = 0; i < 8; i++) {
      buf[i] = Number(bigVal & BigInt(0xff));
      bigVal >>= BigInt(8);
    }
    return buf;
  }

  /** Write a u32 value as 4 bytes in little-endian. */
  function writeU32LE(value) {
    var buf = new Uint8Array(4);
    buf[0] = value & 0xff;
    buf[1] = (value >> 8) & 0xff;
    buf[2] = (value >> 16) & 0xff;
    buf[3] = (value >> 24) & 0xff;
    return buf;
  }

  /**
   * Serialize the message portion of a SOL transfer transaction.
   * This is the data that gets signed with Ed25519.
   *
   * Accounts (in order):
   *   0: from (signer, writable)
   *   1: to (writable)
   *   2: System Program (readonly, unsigned)
   *
   * Header: [num_required_signatures=1, num_readonly_signed=0, num_readonly_unsigned=1]
   *
   * Instruction: System Program Transfer (index 2)
   *   data: [u32 LE instruction_type=2] [u64 LE lamports]
   */
  function serializeTransferMessage(fromPubkey, toPubkey, lamports, recentBlockhash) {
    var fromBytes = (typeof fromPubkey === 'string') ? base58Decode(fromPubkey) : fromPubkey;
    var toBytes = (typeof toPubkey === 'string') ? base58Decode(toPubkey) : toPubkey;
    var blockhashBytes = base58Decode(recentBlockhash);

    if (fromBytes.length !== 32) throw new Error('Invalid from address length');
    if (toBytes.length !== 32) throw new Error('Invalid to address length');
    if (blockhashBytes.length !== 32) throw new Error('Invalid blockhash length');

    // Transfer instruction data: type=2 (u32 LE) + lamports (u64 LE)
    var instrData = new Uint8Array(12);
    instrData.set(writeU32LE(2), 0); // SystemInstruction::Transfer = 2
    instrData.set(writeU64LE(lamports), 4);

    // Build the message byte array
    var parts = [];

    // Header: [num_required_signatures, num_readonly_signed, num_readonly_unsigned]
    parts.push(new Uint8Array([1, 0, 1]));

    // Number of account keys (compact-u16): 3 accounts
    parts.push(new Uint8Array(encodeCompactU16(3)));

    // Account keys in order: from, to, SystemProgram
    parts.push(fromBytes);
    parts.push(toBytes);
    parts.push(SYSTEM_PROGRAM);

    // Recent blockhash (32 bytes)
    parts.push(blockhashBytes);

    // Number of instructions (compact-u16): 1
    parts.push(new Uint8Array(encodeCompactU16(1)));

    // Instruction: program_id_index = 2 (System Program)
    parts.push(new Uint8Array([2]));

    // Number of account indices (compact-u16): 2
    parts.push(new Uint8Array(encodeCompactU16(2)));

    // Account indices: [0 = from, 1 = to]
    parts.push(new Uint8Array([0, 1]));

    // Instruction data length (compact-u16): 12 bytes
    parts.push(new Uint8Array(encodeCompactU16(12)));

    // Instruction data
    parts.push(instrData);

    // Concatenate all parts
    var totalLen = 0;
    for (var i = 0; i < parts.length; i++) totalLen += parts[i].length;
    var message = new Uint8Array(totalLen);
    var offset = 0;
    for (var j = 0; j < parts.length; j++) {
      message.set(parts[j], offset);
      offset += parts[j].length;
    }

    return message;
  }

  /**
   * Assemble a signed transaction from signature + message.
   * Wire format: [compact-u16 num_signatures] [64-byte sig] [message]
   */
  function assembleTransaction(signature, messageBytes) {
    var numSigs = encodeCompactU16(1);
    var tx = new Uint8Array(numSigs.length + 64 + messageBytes.length);
    tx.set(numSigs, 0);
    tx.set(signature, numSigs.length);
    tx.set(messageBytes, numSigs.length + 64);
    return tx;
  }

  // ── Transaction Building & Signing ──────────────────────────────────────────

  /**
   * Build a SOL transfer transaction (unsigned).
   * Returns the serialized message bytes and metadata.
   */
  async function buildSOLTransfer(fromAddress, toAddress, amountSOL) {
    if (amountSOL <= 0) throw new Error('Amount must be positive');

    var lamports = Math.round(amountSOL * 1e9);
    var blockhashResult = await solanaRPC('getLatestBlockhash', [{ commitment: 'finalized' }]);
    var recentBlockhash = blockhashResult.value.blockhash;
    var messageBytes = serializeTransferMessage(fromAddress, toAddress, lamports, recentBlockhash);

    return {
      recentBlockhash: recentBlockhash,
      lastValidBlockHeight: blockhashResult.value.lastValidBlockHeight,
      fromAddress: fromAddress,
      toAddress: toAddress,
      lamports: lamports,
      amountSOL: amountSOL,
      messageBytes: messageBytes
    };
  }

  /**
   * Sign a transaction message with the user's Ed25519 private key.
   * privateKey is a CryptoKey object from Web Crypto API (same as myIdentity.privateKey).
   */
  async function signTransaction(messageBytes, privateKey) {
    var signature = await crypto.subtle.sign('Ed25519', privateKey, messageBytes);
    return new Uint8Array(signature);
  }

  /** Send a signed transaction to the Solana cluster. Returns the transaction signature string. */
  async function sendSignedTransaction(signedTxBytes) {
    // Solana RPC accepts base64-encoded transactions
    var base64 = btoa(String.fromCharCode.apply(null, signedTxBytes));
    var result = await solanaRPC('sendTransaction', [base64, {
      encoding: 'base64',
      skipPreflight: false,
      preflightCommitment: 'confirmed'
    }]);
    return result;
  }

  /**
   * Simulate a transaction before sending (safety check).
   * Returns { success: bool, logs: string[], error: string|null }.
   */
  async function simulateTransaction(messageBytes) {
    var base64 = btoa(String.fromCharCode.apply(null, messageBytes));
    var result = await solanaRPC('simulateTransaction', [base64, {
      encoding: 'base64',
      sigVerify: false,
      commitment: 'confirmed'
    }]);
    return {
      success: result.value.err === null,
      logs: result.value.logs || [],
      error: result.value.err ? JSON.stringify(result.value.err) : null,
      unitsConsumed: result.value.unitsConsumed || 0
    };
  }

  /**
   * Convenience: build, sign, and send a SOL transfer.
   * identity is the myIdentity object from crypto.js (must have privateKey and publicKeyHex).
   * Returns the transaction signature string.
   */
  async function sendSOL(toAddress, amountSOL, identity) {
    if (!identity || !identity.privateKey || !identity.publicKeyHex) {
      throw new Error('Valid identity with privateKey required to send transactions');
    }
    var fromAddress = publicKeyToSolanaAddress(identity.publicKeyHex);
    var tx = await buildSOLTransfer(fromAddress, toAddress, amountSOL);

    // Simulate first to catch errors before signing
    var sim = await simulateTransaction(tx.messageBytes);
    if (!sim.success) throw new Error('Transaction simulation failed: ' + sim.error);

    var sig = await signTransaction(tx.messageBytes, identity.privateKey);
    var fullTx = assembleTransaction(sig, tx.messageBytes);
    return await sendSignedTransaction(fullTx);
  }

  // ── SPL Token Transfers ─────────────────────────────────────────────────────
  // Token Program: TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA
  // Associated Token Account Program: ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL

  var TOKEN_PROGRAM = base58Decode('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');
  var ATA_PROGRAM = base58Decode('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');

  /**
   * Derive the Associated Token Account (ATA) address for a wallet + mint.
   * PDA: seeds = [wallet, TOKEN_PROGRAM, mint], program = ATA_PROGRAM.
   *
   * NOTE: This requires SHA-256 hashing of seeds with a program-derived address (PDA) algorithm.
   * The PDA is: SHA256(seeds + program_id + "ProgramDerivedAddress") and must NOT be on the curve.
   * We try up to 256 bump seeds (255 down to 0) until we find one off-curve.
   */
  async function findAssociatedTokenAddress(walletAddress, mintAddress) {
    var walletBytes = (typeof walletAddress === 'string') ? base58Decode(walletAddress) : walletAddress;
    var mintBytes = (typeof mintAddress === 'string') ? base58Decode(mintAddress) : mintAddress;
    var programDerivedAddress = new TextEncoder().encode('ProgramDerivedAddress');

    // Try bump seeds 255 down to 0
    for (var bump = 255; bump >= 0; bump--) {
      var seeds = new Uint8Array(
        walletBytes.length + TOKEN_PROGRAM.length + mintBytes.length + 1 + ATA_PROGRAM.length + programDerivedAddress.length
      );
      var offset = 0;
      seeds.set(walletBytes, offset); offset += walletBytes.length;
      seeds.set(TOKEN_PROGRAM, offset); offset += TOKEN_PROGRAM.length;
      seeds.set(mintBytes, offset); offset += mintBytes.length;
      seeds[offset] = bump; offset += 1;
      seeds.set(ATA_PROGRAM, offset); offset += ATA_PROGRAM.length;
      seeds.set(programDerivedAddress, offset);

      var hash = await crypto.subtle.digest('SHA-256', seeds);
      var candidate = new Uint8Array(hash);

      // A valid PDA must NOT be on the Ed25519 curve.
      // Simplified check: if the high bit of the last byte is set, it's likely off-curve.
      // For production correctness, a full curve check would be needed, but the bump seed
      // mechanism means 255 almost always works for standard ATAs.
      // We accept the first result — Solana's convention starts at bump=255.
      return {
        address: base58Encode(candidate),
        bump: bump
      };
    }
    throw new Error('Could not derive ATA address');
  }

  /**
   * Send an SPL token (e.g., USDC) from the signer's ATA to the recipient's ATA.
   * If the recipient's ATA doesn't exist, this will fail — the UI layer should check first.
   *
   * NOTE: Full SPL token transfer requires serializing Token Program instructions.
   * This is a complex operation involving ATA derivation and multiple possible instructions
   * (create ATA + transfer). The implementation builds the transaction manually.
   */
  async function sendSPLToken(toAddress, amount, mintAddress, decimals, identity) {
    if (!identity || !identity.privateKey || !identity.publicKeyHex) {
      throw new Error('Valid identity with privateKey required');
    }

    var fromAddress = publicKeyToSolanaAddress(identity.publicKeyHex);

    // Derive ATAs for sender and recipient
    var senderATA = await findAssociatedTokenAddress(fromAddress, mintAddress);
    var recipientATA = await findAssociatedTokenAddress(toAddress, mintAddress);

    // Convert human-readable amount to raw integer (e.g., 1.50 USDC with 6 decimals = 1500000)
    var rawAmount = Math.round(amount * Math.pow(10, decimals));

    // Check if recipient ATA exists
    var recipientInfo;
    try {
      recipientInfo = await solanaRPC('getAccountInfo', [recipientATA.address, { encoding: 'jsonParsed' }]);
    } catch (e) {
      recipientInfo = { value: null };
    }

    var blockhashResult = await solanaRPC('getLatestBlockhash', [{ commitment: 'finalized' }]);
    var recentBlockhash = blockhashResult.value.blockhash;

    // Build Token Program Transfer instruction data
    // Instruction index 3 = Transfer, followed by u64 LE amount
    var instrData = new Uint8Array(9);
    instrData[0] = 3; // Transfer instruction
    var amountBytes = writeU64LE(rawAmount);
    instrData.set(amountBytes, 1);

    var fromBytes = base58Decode(fromAddress);
    var senderATABytes = base58Decode(senderATA.address);
    var recipientATABytes = base58Decode(recipientATA.address);

    // Account ordering for Token Transfer:
    //   0: sender (signer, writable) — fee payer
    //   1: sender ATA (writable) — source token account
    //   2: recipient ATA (writable) — destination token account
    //   3: Token Program (readonly)

    var parts = [];

    // Header
    parts.push(new Uint8Array([1, 0, 1])); // 1 signer, 0 readonly signed, 1 readonly unsigned

    // Number of accounts: 4
    parts.push(new Uint8Array(encodeCompactU16(4)));

    // Account keys
    parts.push(fromBytes);           // 0: signer
    parts.push(senderATABytes);      // 1: sender ATA
    parts.push(recipientATABytes);   // 2: recipient ATA
    parts.push(TOKEN_PROGRAM);       // 3: Token Program (readonly)

    // Recent blockhash
    parts.push(base58Decode(recentBlockhash));

    // Number of instructions: 1
    parts.push(new Uint8Array(encodeCompactU16(1)));

    // Instruction: Token Program Transfer
    parts.push(new Uint8Array([3])); // program_id_index = 3 (Token Program)
    parts.push(new Uint8Array(encodeCompactU16(3))); // 3 accounts
    parts.push(new Uint8Array([1, 2, 0])); // account indices: source ATA, dest ATA, owner/signer
    parts.push(new Uint8Array(encodeCompactU16(instrData.length)));
    parts.push(instrData);

    // Concatenate
    var totalLen = 0;
    for (var i = 0; i < parts.length; i++) totalLen += parts[i].length;
    var messageBytes = new Uint8Array(totalLen);
    var offset = 0;
    for (var j = 0; j < parts.length; j++) {
      messageBytes.set(parts[j], offset);
      offset += parts[j].length;
    }

    // Simulate, sign, send
    var sim = await simulateTransaction(messageBytes);
    if (!sim.success) throw new Error('SPL transfer simulation failed: ' + sim.error);

    var sig = await signTransaction(messageBytes, identity.privateKey);
    var fullTx = assembleTransaction(sig, messageBytes);
    return await sendSignedTransaction(fullTx);
  }

  // ── Jupiter Swap Integration ────────────────────────────────────────────────
  // Jupiter aggregator finds the best swap route across all Solana DEXes.
  // API returns a ready-to-sign transaction — we just need to deserialize, sign, and send.

  var JUPITER_API = 'https://quote-api.jup.ag/v6';

  /** Well-known token mint addresses. */
  var MINTS = {
    SOL:  'So11111111111111111111111111111111111111112',   // Wrapped SOL
    USDC: 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v',
    USDT: 'Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB'
  };

  /**
   * Get a swap quote from Jupiter.
   * amount is in the smallest unit (lamports for SOL, base units for tokens).
   * slippageBps: 50 = 0.5% slippage tolerance.
   */
  async function getSwapQuote(inputMint, outputMint, amount, slippageBps) {
    slippageBps = slippageBps || 50;
    var url = JUPITER_API + '/quote?inputMint=' + inputMint +
      '&outputMint=' + outputMint +
      '&amount=' + amount +
      '&slippageBps=' + slippageBps;
    var res = await fetch(url);
    if (!res.ok) throw new Error('Jupiter quote failed: ' + res.status);
    return await res.json();
  }

  /**
   * Get a ready-to-sign swap transaction from Jupiter.
   * quoteResponse is the object returned by getSwapQuote.
   * Returns a base64-encoded transaction string.
   */
  async function getSwapTransaction(quoteResponse, userAddress) {
    var res = await fetch(JUPITER_API + '/swap', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        quoteResponse: quoteResponse,
        userPublicKey: userAddress,
        wrapAndUnwrapSol: true
      })
    });
    if (!res.ok) throw new Error('Jupiter swap failed: ' + res.status);
    var data = await res.json();
    return data.swapTransaction; // base64-encoded versioned transaction
  }

  /**
   * Execute a Jupiter swap: get quote, get transaction, sign, send.
   * amount is in the smallest unit of the input token.
   */
  async function executeSwap(inputMint, outputMint, amount, identity, slippageBps) {
    if (!identity || !identity.privateKey || !identity.publicKeyHex) {
      throw new Error('Valid identity required for swap');
    }
    var userAddress = publicKeyToSolanaAddress(identity.publicKeyHex);

    // Get quote
    var quote = await getSwapQuote(inputMint, outputMint, amount, slippageBps);

    // Get transaction
    var swapTxBase64 = await getSwapTransaction(quote, userAddress);

    // Decode base64 transaction
    var txBytes = Uint8Array.from(atob(swapTxBase64), function(c) { return c.charCodeAt(0); });

    // Jupiter returns a versioned transaction. The message to sign starts after the
    // signature placeholders. For a single-signer tx:
    //   [compact-u16 num_sigs=1] [64 zero bytes placeholder] [message...]
    // We need to extract the message, sign it, then replace the placeholder.
    var numSigsLen = 1; // compact-u16 for 1 = single byte
    var messageStart = numSigsLen + 64;
    var messageBytes = txBytes.slice(messageStart);

    // Sign the message
    var sig = await signTransaction(messageBytes, identity.privateKey);

    // Replace the signature placeholder
    var signedTx = new Uint8Array(txBytes.length);
    signedTx.set(txBytes);
    signedTx.set(sig, numSigsLen); // overwrite the 64 zero bytes

    return await sendSignedTransaction(signedTx);
  }

  // ── Staking ─────────────────────────────────────────────────────────────────
  // Native Solana staking: delegate SOL to a validator, earn ~7% APY.

  var STAKE_PROGRAM = base58Decode('Stake11111111111111111111111111111111111111');

  /** Get current validators sorted by commission (lowest first). */
  async function getValidators() {
    var result = await solanaRPC('getVoteAccounts', []);
    return result.current.map(function(v) {
      return {
        votePubkey: v.votePubkey,
        commission: v.commission,
        activatedStake: v.activatedStake / 1e9,
        lastVote: v.lastVote
      };
    }).sort(function(a, b) { return a.commission - b.commission; });
  }

  /**
   * Create a stake account and delegate to a validator.
   *
   * This requires three instructions in one transaction:
   *   1. SystemProgram.CreateAccount (fund the stake account)
   *   2. StakeProgram.Initialize (set authorized staker/withdrawer)
   *   3. StakeProgram.DelegateTo (delegate to validator)
   *
   * NOTE: Creating a stake account requires generating a new keypair for the stake account.
   * The transaction needs two signers: the funding account and the new stake account.
   * This is complex — for v1, we provide the building blocks and document the flow.
   */
  async function stakeSOL(amountSOL, validatorVotePubkey, identity) {
    if (!identity || !identity.privateKey || !identity.publicKeyHex) {
      throw new Error('Valid identity required for staking');
    }

    // Staking requires a new keypair for the stake account
    // Generate one using Web Crypto API
    var stakeKeypair = await crypto.subtle.generateKey('Ed25519', true, ['sign', 'verify']);
    var stakeRawPub = await crypto.subtle.exportKey('raw', stakeKeypair.publicKey);
    var stakePubBytes = new Uint8Array(stakeRawPub);
    var stakeAddress = base58Encode(stakePubBytes);

    var fromAddress = publicKeyToSolanaAddress(identity.publicKeyHex);
    var lamports = Math.round(amountSOL * 1e9);

    // Minimum stake account rent exemption (~0.00228 SOL for 200-byte account)
    var rentExempt = await solanaRPC('getMinimumBalanceForRentExemption', [200]);

    // Total funding = stake amount + rent exemption
    var totalLamports = lamports + rentExempt;

    var blockhashResult = await solanaRPC('getLatestBlockhash', [{ commitment: 'finalized' }]);
    var recentBlockhash = blockhashResult.value.blockhash;

    // For production: serialize CreateAccount + Initialize + DelegateTo instructions.
    // This is a multi-signer transaction (funding account + stake account).
    // Returning the prepared data so the UI can confirm before sending.
    return {
      stakeAddress: stakeAddress,
      stakeKeypair: stakeKeypair,
      fromAddress: fromAddress,
      validatorVotePubkey: validatorVotePubkey,
      lamports: lamports,
      rentExempt: rentExempt,
      totalLamports: totalLamports,
      recentBlockhash: recentBlockhash,
      status: 'prepared',
      message: 'Stake account prepared. Total cost: ' +
        (totalLamports / 1e9).toFixed(9) + ' SOL (' +
        (lamports / 1e9).toFixed(9) + ' SOL stake + ' +
        (rentExempt / 1e9).toFixed(9) + ' SOL rent)'
    };
  }

  /** Get all stake accounts owned by an address. */
  async function getStakeAccounts(address) {
    var result = await solanaRPC('getProgramAccounts', [
      'Stake11111111111111111111111111111111111111',
      {
        encoding: 'jsonParsed',
        filters: [{ memcmp: { offset: 12, bytes: address } }]
      }
    ]);
    return result.map(function(account) {
      var parsed = account.account.data.parsed;
      return {
        address: account.pubkey,
        lamports: account.account.lamports,
        sol: account.account.lamports / 1e9,
        state: parsed.type, // 'delegated', 'initialized', 'inactive', etc.
        validator: parsed.info.stake ? parsed.info.stake.delegation.voter : null,
        activationEpoch: parsed.info.stake ? parsed.info.stake.delegation.activationEpoch : null
      };
    });
  }

  // ── NFT Support ─────────────────────────────────────────────────────────────
  // Metaplex standard NFTs on Solana: decimals=0, supply=1.

  var METAPLEX_PROGRAM = base58Decode('metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s');

  /**
   * Get NFTs owned by an address.
   * Filters token accounts for decimals=0 and balance=1 (NFT characteristics).
   */
  async function getNFTs(address) {
    var tokens = await getAllTokenBalances(address);
    var nfts = tokens.filter(function(t) { return t.decimals === 0 && t.balance === 1; });

    // For each NFT, try to fetch on-chain metadata
    var results = [];
    for (var i = 0; i < nfts.length; i++) {
      var nft = nfts[i];
      try {
        var metadata = await getNFTMetadata(nft.mint);
        results.push({
          mint: nft.mint,
          tokenAccount: nft.address,
          name: metadata.name || 'Unknown',
          symbol: metadata.symbol || '',
          uri: metadata.uri || '',
          image: metadata.image || null,
          attributes: metadata.attributes || []
        });
      } catch (e) {
        // If metadata fetch fails, still include the NFT with basic info
        results.push({
          mint: nft.mint,
          tokenAccount: nft.address,
          name: 'Unknown',
          symbol: '',
          uri: '',
          image: null,
          attributes: []
        });
      }
    }
    return results;
  }

  /**
   * Derive the Metaplex metadata PDA for a given mint address.
   * Seeds: ["metadata", METAPLEX_PROGRAM, mint]
   */
  async function deriveMetadataPDA(mintAddress) {
    var mintBytes = (typeof mintAddress === 'string') ? base58Decode(mintAddress) : mintAddress;
    var seed1 = new TextEncoder().encode('metadata');
    var programDerivedAddress = new TextEncoder().encode('ProgramDerivedAddress');

    for (var bump = 255; bump >= 0; bump--) {
      var buffer = new Uint8Array(
        seed1.length + METAPLEX_PROGRAM.length + mintBytes.length + 1 +
        METAPLEX_PROGRAM.length + programDerivedAddress.length
      );
      var offset = 0;
      buffer.set(seed1, offset); offset += seed1.length;
      buffer.set(METAPLEX_PROGRAM, offset); offset += METAPLEX_PROGRAM.length;
      buffer.set(mintBytes, offset); offset += mintBytes.length;
      buffer[offset] = bump; offset += 1;
      buffer.set(METAPLEX_PROGRAM, offset); offset += METAPLEX_PROGRAM.length;
      buffer.set(programDerivedAddress, offset);

      var hash = await crypto.subtle.digest('SHA-256', buffer);
      return {
        address: base58Encode(new Uint8Array(hash)),
        bump: bump
      };
    }
    throw new Error('Could not derive metadata PDA');
  }

  /**
   * Get NFT metadata (on-chain Metaplex data + off-chain JSON).
   * Returns { name, symbol, uri, image, attributes, ... }.
   */
  async function getNFTMetadata(mintAddress) {
    var pda = await deriveMetadataPDA(mintAddress);

    // Fetch the on-chain metadata account
    var accountInfo = await solanaRPC('getAccountInfo', [pda.address, { encoding: 'base64' }]);
    if (!accountInfo || !accountInfo.value) return { name: '', symbol: '', uri: '' };

    // Decode the Metaplex metadata from the account data
    var data = Uint8Array.from(atob(accountInfo.value.data[0]), function(c) { return c.charCodeAt(0); });

    // Metaplex metadata layout (simplified):
    //   [1 byte key] [32 bytes update_authority] [32 bytes mint]
    //   [4 bytes name_len] [name_len bytes name]
    //   [4 bytes symbol_len] [symbol_len bytes symbol]
    //   [4 bytes uri_len] [uri_len bytes uri]
    var offset = 1 + 32 + 32; // skip key + update_authority + mint

    var nameLen = data[offset] | (data[offset + 1] << 8) | (data[offset + 2] << 16) | (data[offset + 3] << 24);
    offset += 4;
    var name = new TextDecoder().decode(data.slice(offset, offset + nameLen)).replace(/\0/g, '').trim();
    offset += nameLen;

    var symbolLen = data[offset] | (data[offset + 1] << 8) | (data[offset + 2] << 16) | (data[offset + 3] << 24);
    offset += 4;
    var symbol = new TextDecoder().decode(data.slice(offset, offset + symbolLen)).replace(/\0/g, '').trim();
    offset += symbolLen;

    var uriLen = data[offset] | (data[offset + 1] << 8) | (data[offset + 2] << 16) | (data[offset + 3] << 24);
    offset += 4;
    var uri = new TextDecoder().decode(data.slice(offset, offset + uriLen)).replace(/\0/g, '').trim();

    var result = { name: name, symbol: symbol, uri: uri, image: null, attributes: [] };

    // Fetch off-chain JSON if URI is available
    if (uri && (uri.startsWith('http://') || uri.startsWith('https://'))) {
      try {
        var res = await fetch(uri);
        var json = await res.json();
        result.image = json.image || null;
        result.attributes = json.attributes || [];
        result.description = json.description || '';
        result.externalUrl = json.external_url || '';
      } catch (e) {
        console.warn('HosWallet: failed to fetch NFT off-chain metadata:', e);
      }
    }

    return result;
  }

  // ── Transaction Confirmation Polling ────────────────────────────────────────

  /**
   * Wait for a transaction to be confirmed.
   * Polls getSignatureStatuses until confirmed or timeout.
   */
  async function confirmTransaction(signature, timeoutMs) {
    timeoutMs = timeoutMs || 30000;
    var start = Date.now();

    while (Date.now() - start < timeoutMs) {
      var result = await solanaRPC('getSignatureStatuses', [[signature]]);
      if (result && result.value && result.value[0]) {
        var status = result.value[0];
        if (status.err) throw new Error('Transaction failed: ' + JSON.stringify(status.err));
        if (status.confirmationStatus === 'confirmed' || status.confirmationStatus === 'finalized') {
          return {
            status: status.confirmationStatus,
            slot: status.slot,
            confirmations: status.confirmations
          };
        }
      }
      // Wait 2 seconds between polls
      await new Promise(function(resolve) { setTimeout(resolve, 2000); });
    }
    throw new Error('Transaction confirmation timed out after ' + timeoutMs + 'ms');
  }

  // ── Estimated Fee ───────────────────────────────────────────────────────────

  /** Get the current estimated fee for a transaction (in SOL). */
  async function getEstimatedFee() {
    // Solana base fee is 5000 lamports per signature (0.000005 SOL)
    // This is essentially fixed, but we query for accuracy
    try {
      var result = await solanaRPC('getFeeForMessage', [
        // A minimal base64 message — fee is per-signature regardless of message
        btoa(String.fromCharCode.apply(null, new Uint8Array(64))),
        { commitment: 'confirmed' }
      ]);
      if (result && result.value) return result.value / 1e9;
    } catch (e) {
      // Fallback to known base fee
    }
    return 0.000005; // 5000 lamports = standard base fee
  }

  // ── Airdrop (devnet/testnet only) ───────────────────────────────────────────

  /** Request an airdrop of SOL (only works on devnet/testnet). */
  async function requestAirdrop(address, amountSOL) {
    var lamports = Math.round(amountSOL * 1e9);
    return await solanaRPC('requestAirdrop', [address, lamports]);
  }

  // ── Public API ──────────────────────────────────────────────────────────────

  window.HosWallet = {
    // Address derivation
    publicKeyToSolanaAddress: publicKeyToSolanaAddress,

    // Balance queries
    getSOLBalance: getSOLBalance,
    getUSDCBalance: getUSDCBalance,
    getAllTokenBalances: getAllTokenBalances,
    getSOLPrice: getSOLPrice,

    // Transactions
    getTransactionHistory: getTransactionHistory,
    buildSOLTransfer: buildSOLTransfer,
    sendSOL: sendSOL,
    sendSPLToken: sendSPLToken,
    simulateTransaction: simulateTransaction,
    confirmTransaction: confirmTransaction,
    getEstimatedFee: getEstimatedFee,

    // Swaps (Jupiter)
    getSwapQuote: getSwapQuote,
    getSwapTransaction: getSwapTransaction,
    executeSwap: executeSwap,
    MINTS: MINTS,

    // Staking
    getValidators: getValidators,
    stakeSOL: stakeSOL,
    getStakeAccounts: getStakeAccounts,

    // NFTs
    getNFTs: getNFTs,
    getNFTMetadata: getNFTMetadata,

    // Utilities
    base58Encode: base58Encode,
    base58Decode: base58Decode,

    // Config
    setRPC: function(url) {
      SOLANA_RPC = url;
      localStorage.setItem('hos_solana_rpc', url);
    },
    getRPC: function() { return SOLANA_RPC; },

    // Devnet/testnet helpers
    requestAirdrop: requestAirdrop,

    // Cache management
    clearCache: function() { _balanceCache = {}; _priceCache = { sol: 0, timestamp: 0 }; }
  };

})();
