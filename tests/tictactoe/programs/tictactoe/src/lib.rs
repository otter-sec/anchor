use anchor_lang::prelude::*;

const BOARD_ITEM_FREE: u8 = 0; // Free slot
const BOARD_ITEM_X: u8 = 1; // Player X
const BOARD_ITEM_O: u8 = 2; // Player O

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

/// Game State
/// 0 - Waiting
/// 1 - XMove
/// 2 - OMove
/// 3 - XWon
/// 4 - OWon
/// 5 - Draw

#[program]
pub mod tictactoe {
    use super::*;

    pub fn initialize_dashboard(ctx: Context<InitializeDashboard>) -> Result<()> {
        let dashboard = &mut ctx.accounts.dashboard;
        dashboard.game_count = 0;
        dashboard.address = *dashboard.to_account_info().key;
        Ok(())
    }

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let dashboard = &mut ctx.accounts.dashboard;
        let game = &mut ctx.accounts.game;
        dashboard.game_count = dashboard.game_count + 1;
        dashboard.latest_game = *game.to_account_info().key;
        game.player_x = *ctx.accounts.player_x.key;
        Ok(())
    }

    pub fn player_join(ctx: Context<PlayerJoin>) -> Result<()> {
        let game = &mut ctx.accounts.game;
        game.player_o = *ctx.accounts.player_o.key;
        game.game_state = 1;
        Ok(())
    }

    #[access_control(PlayerMove::accounts(&ctx, x_or_o, player_move))]
    pub fn player_move(ctx: Context<PlayerMove>, x_or_o: u8, player_move: u8) -> Result<()> {
        let game = &mut ctx.accounts.game;
        game.board[player_move as usize] = x_or_o;
        game.status(x_or_o);
        Ok(())
    }

    pub fn status(ctx: Context<Status>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeDashboard<'info> {
    #[account(zero)]
    dashboard: Account<'info, Dashboard>,
    authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    player_x: Signer<'info>,
    #[account(mut)]
    dashboard: Account<'info, Dashboard>,
    #[account(zero)]
    game: Account<'info, Game>,
}

#[derive(Accounts)]
pub struct PlayerJoin<'info> {
    player_o: Signer<'info>,
    #[account(mut, constraint = game.game_state == 0 && game.player_x != Pubkey::default())]
    game: Account<'info, Game>,
}

#[derive(Accounts)]
pub struct PlayerMove<'info> {
    player: Signer<'info>,
    #[account(mut)]
    game: Account<'info, Game>,
}

#[derive(Accounts)]
pub struct Status<'info> {
    dashboard: Account<'info, Dashboard>,
    game: Account<'info, Game>,
}

impl<'info> PlayerMove<'info> {
    pub fn accounts(ctx: &Context<PlayerMove>, x_or_o: u8, player_move: u8) -> Result<()> {
        if ctx.accounts.game.board[player_move as usize] != 0 {
            return Err(ErrorCode::Illegalmove.into());
        }
        if x_or_o == BOARD_ITEM_X {
            return PlayerMove::player_x_checks(ctx);
        } else if x_or_o == BOARD_ITEM_O {
            return PlayerMove::player_o_checks(ctx);
        } else {
            return Err(ErrorCode::UnexpectedValue.into());
        }
    }

    pub fn player_x_checks(ctx: &Context<PlayerMove>) -> Result<()> {
        if ctx.accounts.game.player_x != *ctx.accounts.player.key {
            return Err(ErrorCode::Unauthorized.into());
        }
        if ctx.accounts.game.game_state != 1 {
            return Err(ErrorCode::Gamestate.into());
        }
        Ok(())
    }

    pub fn player_o_checks(ctx: &Context<PlayerMove>) -> Result<()> {
        if ctx.accounts.game.player_o != *ctx.accounts.player.key {
            return Err(ErrorCode::Unauthorized.into());
        }
        if ctx.accounts.game.game_state != 2 {
            return Err(ErrorCode::Gamestate.into());
        }
        Ok(())
    }
}

#[account]
pub struct Dashboard {
    game_count: u64,
    latest_game: Pubkey,
    address: Pubkey,
}

#[account]
#[derive(Default)]
pub struct Game {
    keep_alive: [u64; 2],
    player_x: Pubkey,
    player_o: Pubkey,
    game_state: u8,
    board: [u8; 9],
}

#[event]
pub struct GameStatus {
    keep_alive: [u64; 2],
    player_x: Pubkey,
    player_o: Pubkey,
    game_state: u8,
    board: [u8; 9],
}

impl From<GameStatus> for Game {
    fn from(status: GameStatus) -> Self {
        Self {
            keep_alive: status.keep_alive,
            player_x: status.player_x,
            player_o: status.player_o,
            game_state: status.game_state,
            board: status.board,
        }
    }
}

impl Game {
    pub fn status(self: &mut Game, x_or_o: u8) {
        let winner =
            // Check rows.
            Game::same(x_or_o, &self.board[0..3])
            || Game::same(x_or_o, &self.board[3..6])
            || Game::same(x_or_o, &self.board[6..9])
            // Check columns.
            || Game::same(x_or_o, &[self.board[0], self.board[3], self.board[6]])
            || Game::same(x_or_o, &[self.board[1], self.board[4], self.board[7]])
            || Game::same(x_or_o, &[self.board[2], self.board[5], self.board[8]])
            // Check both diagonals.
            || Game::same(x_or_o, &[self.board[0], self.board[4], self.board[8]])
            || Game::same(x_or_o, &[self.board[2], self.board[4], self.board[6]]);

        if winner {
            self.game_state = x_or_o + 2;
        } else if self.board.iter().all(|&p| p != BOARD_ITEM_FREE) {
            self.game_state = 5;
        } else {
            if x_or_o == BOARD_ITEM_X {
                self.game_state = 2;
            } else {
                self.game_state = 1;
            }
        }
    }

    pub fn same(x_or_o: u8, triple: &[u8]) -> bool {
        triple.iter().all(|&i| i == x_or_o)
    }
}

#[error_code]
pub enum ErrorCode {
    #[msg("You are not authorized to perform this action.")]
    Unauthorized,
    #[msg("Wrong dashboard")]
    Wrongdashboard,
    #[msg("Wrong expected state")]
    Gamestate,
    #[msg("Dashboard already initialized")]
    Initialized,
    #[msg("Unexpected value")]
    UnexpectedValue,
    #[msg("Illegal move")]
    Illegalmove,
}
