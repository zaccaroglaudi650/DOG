use {
    anchor_lang::{
        prelude::*,
        solana_program::program::invoke_signed,
        system_program,
    },
    anchor_spl::{
        associated_token,
        token,
    },
    mpl_token_metadata::{
        ID as TOKEN_METADATA_ID,
        instruction as token_instruction,
        state::{CollectionDetails, DataV2},
        // assertions::collection::assert_master_edition,
        utils::assert_derivation,
    },
};

pub fn initialize(
    ctx: Context<Initialize>,
    name: String,
    symbol: String,
    base_token_uri: String,
    price_lamports: u64,
) -> Result<()> {
    // set nft pda
    let nft_pda = &mut ctx.accounts.nft_pda;

    nft_pda.creator = ctx.accounts.nft_manager.key();
    nft_pda.name = name;
    nft_pda.symbol = symbol;
    nft_pda.base_token_uri = base_token_uri;
    nft_pda.price_lamports = price_lamports;
    nft_pda.bump = *ctx.bumps.get("nft_pda").unwrap();

    // set collection pda
    let collection_pda = &mut ctx.accounts.collection_pda;

    collection_pda.authority = nft_pda.to_account_info().key();
    collection_pda.bump = *ctx.bumps.get("collection_pda").unwrap();

    Ok(())
}

pub fn set_metadata(
    ctx: Context<SetMetadata>,
    name: String,
    symbol: String,
    base_token_uri: String
) -> Result<()> {
    let nft_pda = &mut ctx.accounts.nft_pda;

    if &nft_pda.creator != ctx.accounts.nft_manager.key {
        return Err(error!(ErrorCode::Unauthorized));
    }

    nft_pda.name = name;
    nft_pda.symbol = symbol;
    nft_pda.base_token_uri = base_token_uri;
    Ok(())
}

pub fn set_price(ctx: Context<SetPrice>, price_lamports: u64) -> Result<()> {
    let nft_pda = &mut ctx.accounts.nft_pda;

    if &nft_pda.creator != ctx.accounts.nft_manager.key {
        return Err(error!(ErrorCode::Unauthorized));
    }

    nft_pda.price_lamports = price_lamports;
    Ok(())
}

pub fn mint_collection(
    ctx: Context<MintCollection>, 
) -> Result<()> {
    let nft_pda = &ctx.accounts.nft_pda;

    if &nft_pda.creator != ctx.accounts.nft_manager.key {
        return Err(error!(ErrorCode::InvalidNftManager));
    }

    let collection_pda = &mut ctx.accounts.collection_pda;

    if &collection_pda.authority != nft_pda.to_account_info().key {
        return Err(error!(ErrorCode::InvalidCollectionAuthority));
    }

    msg!("Creating mint account...");
    system_program::create_account(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            system_program::CreateAccount {
                from: ctx.accounts.mint_authority.to_account_info(),
                to: ctx.accounts.mint.to_account_info(),
            },
        ),
        10000000,
        82,
        &ctx.accounts.token_program.key(),
    )?;

    msg!("Initializing mint account...");
    token::initialize_mint(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::InitializeMint {
                mint: ctx.accounts.mint.to_account_info(),
                rent: ctx.accounts.rent.to_account_info(),
            },
        ),
        0,
        &ctx.accounts.mint_authority.key(),
        Some(&ctx.accounts.mint_authority.key()),
    )?;

    msg!("Creating token account...");
    associated_token::create(
        CpiContext::new(
            ctx.accounts.associated_token_program.to_account_info(),
            associated_token::Create {
                payer: ctx.accounts.mint_authority.to_account_info(),
                associated_token: ctx.accounts.token_account.to_account_info(),
                authority: ctx.accounts.mint_authority.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
                rent: ctx.accounts.rent.to_account_info(),
            },
        ),
    )?;

    msg!("Minting token to token account...");
    token::mint_to(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::MintTo {
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.token_account.to_account_info(),
                authority: ctx.accounts.mint_authority.to_account_info(),
            },
        ),
        1,
    )?;

    let name = nft_pda.name.to_string();
    let symbol = nft_pda.symbol.to_string();
    let uri = nft_pda.base_token_uri.to_string() + &std::string::ToString::to_string("collection.json");

    let nft_manager = ctx.accounts.nft_manager.to_account_info();
    let nft_manager_key = nft_manager.key();

    let creators = vec![
        mpl_token_metadata::state::Creator {
            address: nft_manager_key,
            verified: false,
            share: 100,
        },
    ];

    let seeds = [b"nft_pda".as_ref(), nft_manager_key.as_ref()];
    let bump = assert_derivation(&crate::id(), &nft_pda.to_account_info(), &seeds)?;
    let signer_seeds = [b"nft_pda".as_ref(), nft_manager_key.as_ref(), &[bump]];

    msg!("Creating metadata account...");
    invoke_signed(
        &token_instruction::create_metadata_accounts_v3(
            TOKEN_METADATA_ID, 
            ctx.accounts.metadata.key(), // metadata_account
            ctx.accounts.mint.key(),  // mint_account
            ctx.accounts.mint_authority.key(), // Mint authority
            ctx.accounts.mint_authority.key(), // Payer
            nft_pda.key(), // Update authority
            name, 
            symbol, 
            uri, 
            Some(creators),
            200, // seller_fee_basis_points
            false, // update_authority_is_signer, 
            true, // is_mutable, 
            None, // Option<Collection>
            None, // Option<Uses>
            Some(CollectionDetails::V1 { size: 0 }), // Option<CollectionDetails>
        ),
        &[
            ctx.accounts.metadata.to_account_info(),
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.mint_authority.to_account_info(),
            ctx.accounts.mint_authority.to_account_info(),
            nft_pda.to_account_info(),
            ctx.accounts.rent.to_account_info(),
        ],
        &[&signer_seeds],
    )?;

    msg!("Creating master edition metadata account...");
    invoke_signed(
        &token_instruction::create_master_edition_v3(
            TOKEN_METADATA_ID, 
            ctx.accounts.master_edition.key(), // // (master) edition account
            ctx.accounts.mint.key(), // mint account
            nft_pda.key(), // Update authority
            ctx.accounts.mint_authority.key(), // Mint authority
            ctx.accounts.metadata.key(), // Metadata
            ctx.accounts.mint_authority.key(), // Payer
            Some(0), // max_supply: Option<u64>
        ),
        &[
            ctx.accounts.master_edition.to_account_info(),
            ctx.accounts.mint.to_account_info(),
            nft_pda.to_account_info(),
            ctx.accounts.mint_authority.to_account_info(),
            ctx.accounts.metadata.to_account_info(),
            ctx.accounts.mint_authority.to_account_info(),
            ctx.accounts.rent.to_account_info(),
        ],
        &[&signer_seeds],
    )?;

    collection_pda.mint = ctx.accounts.mint.key();

    msg!("Token mint process completed successfully.");

    Ok(())
}

pub fn mint(
    ctx: Context<MintNft>,
    token_id: u64,
) -> Result<()> {
    if !(token_id == 1 || token_id == 2) {
        return Err(error!(ErrorCode::InvalidTokenId));
    }

    let nft_pda = &ctx.accounts.nft_pda;

    if &nft_pda.creator != ctx.accounts.nft_manager.key {
        return Err(error!(ErrorCode::InvalidNftManager));
    }

    let collection_pda = &ctx.accounts.collection_pda;

    if &collection_pda.mint != ctx.accounts.collection_mint.key {
        return Err(error!(ErrorCode::InvalidCollectionMint)); 
    }

    msg!("Initiating transfer of {} lamports...", nft_pda.price_lamports);
    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.payer.to_account_info(),
                to: nft_pda.to_account_info(),
            }
        ),
        nft_pda.price_lamports
    )?;

    msg!("Creating mint account...");
    system_program::create_account(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            system_program::CreateAccount {
                from: ctx.accounts.payer.to_account_info(),
                to: ctx.accounts.mint.to_account_info(),
            },
        ),
        10000000, // Lamports
        82, // Size
        &ctx.accounts.token_program.key(), // Token Program owns the Mint account
    )?;

    msg!("Initializing mint account...");
    token::initialize_mint(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::InitializeMint {
                mint: ctx.accounts.mint.to_account_info(),
                rent: ctx.accounts.rent.to_account_info(),
            },
        ),
        0, // Decimals
        &ctx.accounts.mint_authority.key(), // authority
        Some(&ctx.accounts.mint_authority.key()), // freeze authority
    )?;

    msg!("Creating token account...");
    associated_token::create(
        CpiContext::new(
            ctx.accounts.associated_token_program.to_account_info(),
            associated_token::Create {
                payer: ctx.accounts.payer.to_account_info(),
                associated_token: ctx.accounts.token_account.to_account_info(),
                authority: ctx.accounts.payer.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
                rent: ctx.accounts.rent.to_account_info(),
            },
        ),
    )?;

    msg!("Minting token to token account...");
    token::mint_to(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::MintTo {
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.token_account.to_account_info(),
                authority: ctx.accounts.mint_authority.to_account_info(),
            },
        ),
        1,
    )?;

    let name = nft_pda.name.to_string();
    let symbol = nft_pda.symbol.to_string();
    let uri = nft_pda.base_token_uri.to_string() + &token_id.to_string() + &std::string::ToString::to_string(".json");

    let nft_manager = ctx.accounts.nft_manager.to_account_info();
    let nft_manager_key = nft_manager.key();

    let creators = vec![
        mpl_token_metadata::state::Creator {
            address: nft_manager_key,
            verified: false,
            share: 100,
        },
    ];

    let seeds = [b"nft_pda".as_ref(), nft_manager_key.as_ref()];
    let bump = assert_derivation(&crate::id(), &nft_pda.to_account_info(), &seeds)?;
    let signer_seeds = [b"nft_pda".as_ref(), nft_manager_key.as_ref(), &[bump]];

    msg!("Creating metadata account...");
    invoke_signed(
        &token_instruction::create_metadata_accounts_v3(
            TOKEN_METADATA_ID, 
            ctx.accounts.metadata.key(), // metadata_account
            ctx.accounts.mint.key(),  // mint_account
            ctx.accounts.mint_authority.key(), // Mint authority
            ctx.accounts.mint_authority.key(), // Payer
            nft_pda.key(), // Update authority
            name, 
            symbol, 
            uri, 
            Some(creators),
            200, // seller_fee_basis_points
            false, // update_authority_is_signer, 
            true, // is_mutable,
            None, // Option<Collection>
            None, // Option<Uses>
            None, // Option<CollectionDetails>
        ),
        &[
            ctx.accounts.metadata.to_account_info(),
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.mint_authority.to_account_info(),
            ctx.accounts.mint_authority.to_account_info(),
            nft_pda.to_account_info(),
            ctx.accounts.rent.to_account_info(),
        ],
        &[&signer_seeds],
    )?;

    msg!("Creating master edition metadata account...");
    invoke_signed(
        &token_instruction::create_master_edition_v3(
            TOKEN_METADATA_ID, 
            ctx.accounts.master_edition.key(), // // (master) edition account
            ctx.accounts.mint.key(), // mint account
            nft_pda.key(), // Update authority
            ctx.accounts.mint_authority.key(), // Mint authority
            ctx.accounts.metadata.key(), // Metadata
            ctx.accounts.mint_authority.key(), // Payer
            Some(0), // max_supply: Option<u64>
        ),
        &[
            ctx.accounts.master_edition.to_account_info(),
            ctx.accounts.mint.to_account_info(),
            nft_pda.to_account_info(),
            ctx.accounts.mint_authority.to_account_info(),
            ctx.accounts.metadata.to_account_info(),
            ctx.accounts.mint_authority.to_account_info(),
            ctx.accounts.rent.to_account_info(),
        ],
        &[&signer_seeds],
    )?;

    let collection_seeds = [b"collection_pda".as_ref(), nft_manager_key.as_ref()];
    let collection_bump = assert_derivation(&crate::id(), &collection_pda.to_account_info(), &collection_seeds)?;
    let collection_signer_seeds = [b"collection_pda".as_ref(), nft_manager_key.as_ref(), &[collection_bump]];

    msg!("Set and verify collection...");
    invoke_signed(
        &token_instruction::set_and_verify_sized_collection_item(
            TOKEN_METADATA_ID,
            ctx.accounts.metadata.key(), // Metadata account
            collection_pda.key(), // Collection Update authority
            ctx.accounts.payer.key(), // payer
            ctx.accounts.nft_pda.to_account_info().key(), // Update Authority of Collection NFT and NFT
            ctx.accounts.collection_mint.key(), // Mint of the Collection
            ctx.accounts.collection_metadata.key(), // Metadata Account of the Collection
            ctx.accounts.collection_master_edition.key(), // MasterEdition Account of the Collection Token
            Some(ctx.accounts.collection_authority_record.key()), // Collection authority record
        ),
        &[
            ctx.accounts.metadata.to_account_info(),
            collection_pda.to_account_info(),
            ctx.accounts.payer.to_account_info(),
            ctx.accounts.nft_pda.to_account_info(),
            ctx.accounts.collection_mint.to_account_info(),
            ctx.accounts.collection_metadata.to_account_info(),
            ctx.accounts.collection_master_edition.to_account_info(),
            ctx.accounts.collection_authority_record.to_account_info(),
        ],
        &[&collection_signer_seeds],
    )?;

    msg!("Token mint process completed successfully.");

    Ok(())
}

pub fn set_collection(ctx: Context<SetCollection>) -> Result<()> {
    let nft_pda = &ctx.accounts.nft_pda;
    if &nft_pda.creator != ctx.accounts.nft_manager.key {
        return Err(error!(ErrorCode::InvalidNftManager));
    }

    // TODO: check mint is metadata mint

    let authority_record = ctx.accounts.collection_authority_record.to_account_info();
    let nft_manager_key = ctx.accounts.nft_manager.key();

    let seeds = [b"nft_pda".as_ref(), nft_manager_key.as_ref()];
    let bump = assert_derivation(&crate::id(), &nft_pda.to_account_info(), &seeds)?;
    let signer_seeds = [b"nft_pda".as_ref(), nft_manager_key.as_ref(), &[bump]];

    msg!("Approve collection authority...");
    if authority_record.data_is_empty() {
        invoke_signed(
            &token_instruction::approve_collection_authority(
                TOKEN_METADATA_ID,
                authority_record.key(),
                ctx.accounts.collection_pda.to_account_info().key(),
                ctx.accounts.nft_pda.to_account_info().key(),
                ctx.accounts.payer.key(),
                ctx.accounts.metadata.key(),
                ctx.accounts.mint.key(),
            ),
            &[
                authority_record.clone(),
                ctx.accounts.collection_pda.to_account_info(),
                ctx.accounts.nft_pda.to_account_info(),
                ctx.accounts.payer.to_account_info(),
                ctx.accounts.metadata.to_account_info(),
                ctx.accounts.mint.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
                ctx.accounts.rent.to_account_info(),
            ],
            &[&signer_seeds],
        )?;
    }

    Ok(())
}

pub fn set_and_verify_collection(
    ctx: Context<SetAndVerifyCollection>,
) -> Result<()> {
    let nft_pda = &ctx.accounts.nft_pda;

    if &nft_pda.creator != ctx.accounts.nft_manager.key {
        return Err(error!(ErrorCode::InvalidNftManager));
    }

    let collection_pda = &ctx.accounts.collection_pda;

    if &collection_pda.mint != ctx.accounts.collection_mint.key {
        return Err(error!(ErrorCode::InvalidCollectionMint)); 
    }

    let nft_manager_key = ctx.accounts.nft_manager.key();
    let collection_mint = ctx.accounts.collection_mint.to_account_info();

    let collection_seeds = [b"collection_pda".as_ref(), nft_manager_key.as_ref()];
    let collection_bump = assert_derivation(&crate::id(), &collection_pda.to_account_info(), &collection_seeds)?;
    let collection_signer_seeds = [b"collection_pda".as_ref(), nft_manager_key.as_ref(), &[collection_bump]];

    msg!("Set and verify collection...");
    invoke_signed(
        &token_instruction::set_and_verify_sized_collection_item(
            TOKEN_METADATA_ID,
            ctx.accounts.metadata.key(), // Metadata account
            collection_pda.key(), // Collection Update authority
            ctx.accounts.payer.key(), // payer
            ctx.accounts.nft_pda.to_account_info().key(), // Update Authority of Collection NFT and NFT
            collection_mint.key(), // Mint of the Collection
            ctx.accounts.collection_metadata.key(), // Metadata Account of the Collection
            ctx.accounts.collection_master_edition.key(), // MasterEdition Account of the Collection Token
            Some(ctx.accounts.collection_authority_record.key()), // Collection authority record
        ),
        &[
            ctx.accounts.metadata.to_account_info(),
            collection_pda.to_account_info(),
            ctx.accounts.payer.to_account_info(),
            ctx.accounts.nft_pda.to_account_info(),
            collection_mint.to_account_info(),
            ctx.accounts.collection_metadata.to_account_info(),
            ctx.accounts.collection_master_edition.to_account_info(),
            ctx.accounts.collection_authority_record.to_account_info(),
        ],
        &[&collection_signer_seeds],
    )?;

  Ok(())
}

pub fn update_metadata_account(
    ctx: Context<UpdateMetadataAccount>,
    name: String,
    symbol: String,
    uri: String,
) -> Result<()> {
    let nft_pda = &ctx.accounts.nft_pda;

    if &nft_pda.creator != ctx.accounts.nft_manager.key {
        return Err(error!(ErrorCode::Unauthorized));
    }

    let nft_manager = ctx.accounts.nft_manager.to_account_info();
    let nft_manager_key = nft_manager.key();

    let creators = vec![
        mpl_token_metadata::state::Creator {
            address: nft_manager_key,
            verified: false,
            share: 100,
        },
    ];

    let data = DataV2 {
        name,
        symbol,
        uri,
        seller_fee_basis_points: 200, // seller_fee_basis_points
        creators: Some(creators),
        collection: None, // Option<Collection>
        uses: None, // Option<Uses>
    };

    let seeds = [b"nft_pda".as_ref(), nft_manager_key.as_ref()];
    let bump = assert_derivation(&crate::id(), &nft_pda.to_account_info(), &seeds)?;
    let signer_seeds = [b"nft_pda".as_ref(), nft_manager_key.as_ref(), &[bump]];

    msg!("Updating metadata account...");
    invoke_signed(
        &token_instruction::update_metadata_accounts_v2(
            TOKEN_METADATA_ID, 
            ctx.accounts.metadata.key(), // metadata_account
            nft_pda.to_account_info().key(), // update authority
            None, // new update authority
            Some(data), // data
            None, // primary_sale_happened
            Some(true), // is_mutable
        ),
        &[
            ctx.accounts.metadata.to_account_info(),
            nft_pda.to_account_info(),
        ],
        &[&signer_seeds],
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub initializer: Signer<'info>,
    // space: 8 discriminator
    // + 32 creator
    // + 4 name length + 100 name
    // + 4 name length + 100 symbol
    // + 4 name length + 200 base_token_uri
    // + 1 bump
    #[account(
        init,
        payer = initializer,
        space = 453,
        seeds = [b"nft_pda".as_ref(), nft_manager.to_account_info().key.as_ref()],
        bump,
    )]
    pub nft_pda: Account<'info, NftPda>,
    // space: 8 discriminator
    // + 32 authority
    // + 32 mint
    // + 1 bump
    #[account(
        init,
        payer = initializer,
        space = 73,
        seeds = [b"collection_pda".as_ref(), nft_manager.to_account_info().key.as_ref()],
        bump,
    )]
    pub collection_pda: Account<'info, CollectionPda>,
    /// CHECK: We're about to create this with Anchor
    #[account(mut)]
    pub nft_manager: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetMetadata<'info> {
    #[account(mut, seeds = [b"nft_pda".as_ref(), nft_manager.to_account_info().key.as_ref()], bump)]
    pub nft_pda: Account<'info, NftPda>,
    #[account(mut)]
    pub nft_manager: Signer<'info>,
}

#[derive(Accounts)]
pub struct SetPrice<'info> {
    #[account(mut, seeds = [b"nft_pda".as_ref(), nft_manager.to_account_info().key.as_ref()], bump)]
    pub nft_pda: Account<'info, NftPda>,
    #[account(mut)]
    pub nft_manager: Signer<'info>,
}

#[derive(Accounts)]
pub struct MintCollection<'info> {
    #[account(mut, seeds = [b"nft_pda".as_ref(), nft_manager.to_account_info().key.as_ref()], bump)]
    pub nft_pda: Account<'info, NftPda>,
    #[account(mut, seeds = [b"collection_pda".as_ref(), nft_manager.to_account_info().key.as_ref()], bump)]
    pub collection_pda: Account<'info, CollectionPda>,
    /// CHECK: We're about to create this with Metaplex
    #[account(mut)]
    pub metadata: UncheckedAccount<'info>,
    /// CHECK: We're about to create this with Metaplex
    #[account(mut)]
    pub master_edition: UncheckedAccount<'info>,
    #[account(mut)]
    pub mint: Signer<'info>,
    /// CHECK: We're about to create this with Anchor
    #[account(mut)]
    pub token_account: UncheckedAccount<'info>,
    #[account(mut)]
    pub mint_authority: Signer<'info>,
    /// CHECK: We're about to create this with Anchor
    #[account(mut)]
    pub nft_manager: UncheckedAccount<'info>,
    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, token::Token>,
    pub associated_token_program: Program<'info, associated_token::AssociatedToken>,
    /// CHECK: Metaplex will check this
    pub token_metadata_program: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct MintNft<'info> {
    #[account(mut, seeds = [b"nft_pda".as_ref(), nft_manager.to_account_info().key.as_ref()], bump)]
    pub nft_pda: Account<'info, NftPda>,
    #[account(mut, seeds = [b"collection_pda".as_ref(), nft_manager.to_account_info().key.as_ref()], bump)]
    pub collection_pda: Account<'info, CollectionPda>,
    /// CHECK: We're about to create this with Metaplex
    #[account(mut)]
    pub metadata: UncheckedAccount<'info>,
    /// CHECK: We're about to create this with Metaplex
    #[account(mut)]
    pub master_edition: UncheckedAccount<'info>,
    #[account(mut)]
    pub mint: Signer<'info>,
    /// CHECK: We're about to create this with Anchor
    #[account(mut)]
    pub token_account: UncheckedAccount<'info>,
    /// CHECK: We're about to create this with Anchor
    #[account(mut)]
    pub mint_authority: UncheckedAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: We're about to create this with Anchor
    #[account(mut)]
    pub nft_manager: UncheckedAccount<'info>,
    /// CHECK: We're about to create this with Anchor
    pub collection_mint: UncheckedAccount<'info>,
    /// CHECK: We're about to create this with Metaplex
    #[account(mut)]
    pub collection_metadata: UncheckedAccount<'info>,
    /// CHECK: We're about to create this with Metaplex
    #[account(mut)]
    pub collection_master_edition: UncheckedAccount<'info>,
    /// CHECK: We're about to create this with Metaplex
    pub collection_authority_record: UncheckedAccount<'info>,
    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, token::Token>,
    pub associated_token_program: Program<'info, associated_token::AssociatedToken>,
    /// CHECK: Metaplex will check this
    pub token_metadata_program: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct SetCollection<'info> {
    #[account(mut, seeds = [b"nft_pda".as_ref(), nft_manager.to_account_info().key.as_ref()], bump)]
    pub nft_pda: Account<'info, NftPda>,
    /// CHECK: account constraints checked in account trait
    #[account(mut, seeds = [b"collection_pda".as_ref(), nft_manager.to_account_info().key.as_ref()], bump)]
    pub collection_pda: UncheckedAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: We're about to create this with Metaplex
    pub metadata: UncheckedAccount<'info>,
    /// CHECK: We're about to create this with Anchor
    pub mint: UncheckedAccount<'info>,
    /// CHECK: We're about to create this with Anchor
    pub edition: UncheckedAccount<'info>,
    /// CHECK: We're about to create this with Anchor
    #[account(mut)]
    pub collection_authority_record: UncheckedAccount<'info>,
    /// CHECK: We're about to create this with Anchor
    #[account(mut)]
    pub nft_manager: UncheckedAccount<'info>,
    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
    /// CHECK: Metaplex will check this
    pub token_metadata_program: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct SetAndVerifyCollection<'info> {
    #[account(mut, seeds = [b"nft_pda".as_ref(), nft_manager.to_account_info().key.as_ref()], bump)]
    pub nft_pda: Account<'info, NftPda>,
    /// CHECK: We're about to create this with Metaplex
    #[account(mut)]
    pub metadata: UncheckedAccount<'info>,
    #[account(mut, seeds = [b"collection_pda".as_ref(), nft_manager.to_account_info().key.as_ref()], bump)]
    pub collection_pda: Account<'info, CollectionPda>,
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: We're about to create this with Anchor
    #[account(mut)]
    pub nft_manager: UncheckedAccount<'info>,
    /// CHECK: We're about to create this with Anchor
    pub collection_mint: UncheckedAccount<'info>,
    /// CHECK: We're about to create this with Metaplex
    #[account(mut)]
    pub collection_metadata: UncheckedAccount<'info>,
    /// CHECK: We're about to create this with Metaplex
    #[account(mut)]
    pub collection_master_edition: UncheckedAccount<'info>,
    /// CHECK: We're about to create this with Metaplex
    pub collection_authority_record: UncheckedAccount<'info>,
    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
    /// CHECK: Metaplex will check this
    pub token_metadata_program: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct UpdateMetadataAccount<'info> {
    #[account(mut, seeds = [b"nft_pda".as_ref(), nft_manager.to_account_info().key.as_ref()], bump)]
    pub nft_pda: Account<'info, NftPda>,
    /// CHECK: We're about to create this with Metaplex
    #[account(mut)]
    pub metadata: UncheckedAccount<'info>,
    #[account(mut)]
    pub nft_manager: Signer<'info>,
    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
    /// CHECK: Metaplex will check this
    pub token_metadata_program: UncheckedAccount<'info>,
}

#[account]
pub struct NftPda {
    pub creator: Pubkey,
    pub name: String,
    pub symbol: String,
    pub base_token_uri: String,
    pub price_lamports: u64,
    pub bump: u8,
}

#[account]
pub struct CollectionPda {
    pub authority: Pubkey,
    pub mint: Pubkey,
    pub bump: u8,
}

#[error_code]
pub enum ErrorCode {
    #[msg("You are not authorized to perform this action.")]
    Unauthorized,
    #[msg("Invalid nft manager.")]
    InvalidNftManager,
    #[msg("Invalid collection authority.")]
    InvalidCollectionAuthority,
    #[msg("Invalid collection mint.")]
    InvalidCollectionMint,
    #[msg("Invalid token id.")]
    InvalidTokenId,
}
