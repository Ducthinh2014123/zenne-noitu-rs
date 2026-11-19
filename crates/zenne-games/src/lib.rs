pub mod afk;
pub mod anagram;
pub mod battleship;
pub mod baucua;
pub mod cadao;
pub mod chinhta;
pub mod connect4;
pub mod emoji;
pub mod flag_quiz;
pub mod higher_lower;
pub mod history_guess;
pub mod mastermind;
pub mod memory;
pub mod quiz;
pub mod taixiu;
pub mod vuatiengviet;
pub mod whoami;
pub mod wordle;

use afk::AfkManager;
use anagram::AnagramManager;
use battleship::{BattleshipManager, SudokuManager};
use cadao::CaDaoManager;
use chinhta::ChinhTaManager;
use connect4::{Connect4Manager, TypingTestManager};
use emoji::EmojiGameManager;
use flag_quiz::FlagQuizManager;
use higher_lower::HigherLowerManager;
use history_guess::HistoryGuessManager;
use mastermind::MastermindManager;
use memory::{MemoryManager, UnoManager};
use quiz::QuizManager;
use vuatiengviet::VuaTiengVietManager;
use whoami::WhoAmIManager;
use wordle::WordleManager;

#[derive(Clone, Default)]
pub struct GameHub {
    pub afk: AfkManager,
    pub anagram: AnagramManager,
    pub battleship: BattleshipManager,
    pub cadao: CaDaoManager,
    pub chinhta: ChinhTaManager,
    pub connect4: Connect4Manager,
    pub emoji: EmojiGameManager,
    pub flag_quiz: FlagQuizManager,
    pub higher_lower: HigherLowerManager,
    pub history_guess: HistoryGuessManager,
    pub mastermind: MastermindManager,
    pub memory: MemoryManager,
    pub quiz: QuizManager,
    pub sudoku: SudokuManager,
    pub typing: TypingTestManager,
    pub uno: UnoManager,
    pub vuatiengviet: VuaTiengVietManager,
    pub whoami: WhoAmIManager,
    pub wordle: WordleManager,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minigames_preserved_and_functional() {
        let hub = GameHub::default();
        let tx = taixiu::roll_taixiu();
        assert!((3..=18).contains(&tx.total));

        let bc = baucua::roll_baucua();
        assert_eq!(bc.len(), 3);

        let vtv = hub.vuatiengviet.start(100);
        assert!(!vtv.scrambled.is_empty());
        assert_eq!(
            hub.vuatiengviet.check_answer(100, &vtv.answer),
            Some((true, vtv.answer.clone()))
        );

        let q = hub.quiz.start_general(200);
        let letter = ["A", "B", "C", "D"][q.correct_index];
        assert!(hub.quiz.answer(200, letter).unwrap().0);

        let target = hub.wordle.start(300);
        let (_, won, _, _, _) = hub.wordle.guess(300, &target).unwrap();
        assert!(won);
    }
}
