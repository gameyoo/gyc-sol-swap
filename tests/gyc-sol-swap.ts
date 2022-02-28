import * as anchor from '@project-serum/anchor';
import { Program, BN } from '@project-serum/anchor';
import { GycSolSwap } from '../target/types/gyc_sol_swap';

import assert from 'assert';

import {
  clusterApiUrl,
  Transaction,
  sendAndConfirmTransaction,
  Message,
  Connection, 
  LAMPORTS_PER_SOL,
  PublicKey, 
  Keypair, 
  SystemProgram, 
  SYSVAR_RENT_PUBKEY, 
  SYSVAR_CLOCK_PUBKEY,
} from "@solana/web3.js";

import {
  createApproveCheckedInstruction,
  getAccount,
  createAssociatedTokenAccount,
  createMint,
  mintTo,
  getAssociatedTokenAddress,
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,

} from "@solana/spl-token";

import * as nacl from "tweetnacl";
import * as bs58 from "bs58";

import provider from '@project-serum/anchor/dist/cjs/provider';
import provider from '@project-serum/anchor/dist/cjs/provider';

describe('gyc-sol-swap', () => {

  const provider = anchor.Provider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.GycSolSwap as Program<GycSolSwap>;

  const seed = "gyc-sol-swap";

  const updateKey  = Keypair.generate();
  console.log("updateKey: ", updateKey.publicKey.toString());

  let solVaultKey: PublicKey;
  let configKey: PublicKey;
  let vaultNonce: number;
  let configNonce: number;
  let tokenVaultKey: PublicKey;
  let mint: PublicKey;

  const userKey  = Keypair.generate();
  let userTokenKey: PublicKey;

  const userTokenAmount = 10000000000;

  before(async () => {

    [configKey, configNonce] = await PublicKey.findProgramAddress(
      [Buffer.from(seed)],
      program.programId
    );
    console.log("configKey: ", configKey.toString());

    [solVaultKey, vaultNonce] = await PublicKey.findProgramAddress(
      [configKey.toBuffer()],
      program.programId
    );
    console.log("solVaultKey: ", solVaultKey.toString());

    mint = await createMint(provider.connection,
      provider.wallet.payer,
      provider.wallet.publicKey,
      provider.wallet.publicKey,
      9,
      );
    
    console.log("mint: ",mint.toString());

    tokenVaultKey = await getAssociatedTokenAddress(
      mint,
      solVaultKey,
      true,
    );
    console.log("tokenVaultKey: ", tokenVaultKey.toString());

    userTokenKey = await createAssociatedTokenAccount(
      provider.connection,
      provider.wallet.payer,
      mint,
      userKey.publicKey,
    );
    console.log("userKey: ", userKey.publicKey.toString());
    console.log("userTokenKey: ", userTokenKey.toString());

    await mintTo(
      provider.connection,
      provider.wallet.payer,
      mint,
      userTokenKey,
      provider.wallet.publicKey,
      userTokenAmount,
    );

    //const userAccInfo = await provider.connection.getAccountInfo(userKey.publicKey);
    const userBalance = await provider.connection.getBalance(userKey.publicKey);
    console.log("user lamports balance: ", userBalance);
    const tokenBalance = await provider.connection.getTokenAccountBalance(userTokenKey);
    console.log("user Token balance: ", tokenBalance);
    
    assert.strictEqual(tokenBalance.value.amount, String(userTokenAmount));

  });

  it('Is initialized!', async () => {

    const tx = await program.rpc.initialize(
      configNonce,
      vaultNonce,
      updateKey.publicKey,
      {
        accounts: {
          signer: provider.wallet.publicKey,
          config: configKey,
          solVault: solVaultKey,
          tokenVault: tokenVaultKey,
          mint: mint,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
          rent: SYSVAR_RENT_PUBKEY,
          clock: SYSVAR_CLOCK_PUBKEY,
        },
        signers: [provider.wallet.payer],
      });
    console.log("tx: ", tx);
    const ConfigAccount = await program.account.swapSettings.fetch(configKey);
    console.log("authority: ", ConfigAccount.authority.toBase58());

    console.log("initializer: ", provider.wallet.publicKey.toString());

    assert.strictEqual(ConfigAccount.authority.toBase58(), updateKey.publicKey.toBase58());

  });

  
  it('update price!', async () => {

    const signature = await provider.connection.requestAirdrop(updateKey.publicKey, LAMPORTS_PER_SOL);
    await provider.connection.confirmTransaction(signature);

    const tx = await program.rpc.updatePrice(
      new BN(3),
      new BN(1000),
      {
        accounts: {
          signer: updateKey.publicKey,
          config: configKey,
          clock: SYSVAR_CLOCK_PUBKEY,
        },
        signers: [updateKey],
      });
    console.log("tx: ", tx);

    const ConfigAccount = await program.account.swapSettings.fetch(configKey);
    console.log("authority: ", ConfigAccount.authority.toBase58());
    console.log("gyc price: ", ConfigAccount.gycPrice.toString());
    console.log("sol price: ", ConfigAccount.solPrice.toString());
    console.log("timestamp: ", ConfigAccount.timestamp.toString());

    assert.strictEqual(ConfigAccount.gycPrice.toString(), String(3));
    assert.strictEqual(ConfigAccount.solPrice.toString(), String(1000));

  });

  it('approve token account!', async () => {
    let tx = new Transaction().add(
      createApproveCheckedInstruction(
          userTokenKey,
          mint,
          solVaultKey,
          userKey.publicKey,
          1e9 * 1000,
          9
      )
    );
    tx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
    tx.feePayer = updateKey.publicKey;
    let realDataNeedToSign = tx.serializeMessage();

    let approverSignature = nacl.sign.detached(realDataNeedToSign, userKey.secretKey);
    
    let feePayerSignature = nacl.sign.detached(realDataNeedToSign, updateKey.secretKey);
    let recoverTx = Transaction.populate(Message.from(realDataNeedToSign),[
      bs58.encode(feePayerSignature),
      bs58.encode(approverSignature)
    ]);

    //recoverTx.addSignature(updateKey.publicKey, Buffer.from(feePayerSignature));
    const txSignature = await provider.connection.sendRawTransaction(recoverTx.serialize());
    await provider.connection.confirmTransaction(txSignature);
    console.log("txSignature: ", txSignature);

    const tokenAccount = await getAccount(provider.connection, userTokenKey,);
    console.log(`
        token address: ${tokenAccount.address.toBase58()}
        amount: ${tokenAccount.amount}
        closeAuthority: ${tokenAccount.closeAuthority?.toBase58()}
        delegate: ${tokenAccount.delegate?.toBase58()}
        delegatedAmount: ${tokenAccount.delegatedAmount}
        mint: ${tokenAccount.mint.toBase58()}
        owner: ${tokenAccount.owner.toBase58()}
    `);


  });

  it('gyc to sol!', async () => {
    const signature = await provider.connection.requestAirdrop(solVaultKey, LAMPORTS_PER_SOL);
    await provider.connection.confirmTransaction(signature);

    const tx = await program.rpc.gycToSol(
      new BN(userTokenAmount),
      {
        accounts: {
          signer: updateKey.publicKey,
          recipient: userKey.publicKey,
          recipientToken: userTokenKey,
          mint: mint,
          solVault: solVaultKey,
          tokenVault: tokenVaultKey,
          config: configKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        },
        signers: [updateKey],
      });
      console.log("tx: ", tx);

      const userBalance = await provider.connection.getBalance(userKey.publicKey);
      console.log("user lamports balance: ", userBalance);
      const tokenBalance = await provider.connection.getTokenAccountBalance(userTokenKey);
      console.log("user Token balance: ", tokenBalance);

  });

  it('withdraw token!', async () => {
    const walletTokenKey = await createAssociatedTokenAccount(
      provider.connection,
      provider.wallet.payer,
      mint,
      provider.wallet.publicKey,
    );
    console.log("wallet publicKey: ", provider.wallet.publicKey.toString());
    console.log("walletTokenKey: ", walletTokenKey.toString());
    
    const tokenBalance = await provider.connection.getTokenAccountBalance(tokenVaultKey);
    console.log("Token vault balance: ", tokenBalance);

    const tx = await program.rpc.withdraw(
      new BN(tokenBalance.value.amount),
      {
        accounts: {
          signer: provider.wallet.publicKey,
          recipientToken: walletTokenKey,
          solVault: solVaultKey,
          tokenVault: tokenVaultKey,
          config: configKey,
          mint: mint,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [provider.wallet.payer]
      }
    );
    console.log("tx: ",tx);

    const walletTokenBalance = await provider.connection.getTokenAccountBalance(walletTokenKey);
    console.log("Wallet Token balance: ", walletTokenBalance);



  });



});
