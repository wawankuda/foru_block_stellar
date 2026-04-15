#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String, Vec};

// ================================================
// 1. KUNCI PENYIMPANAN (Storage Keys) – Menggunakan Enum
// Ini adalah "relasi database" di blockchain seperti yang kamu inginkan.
// Tidak lagi pakai Symbol sederhana NOTE_DATA.
// ================================================
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    AllNotes,                  // Vec<u64> → daftar ID semua notes (untuk get_notes)
    Note(u64),                 // Data Note tunggal berdasarkan ID
    NoteComments(u64),         // Vec<Comment> → komentar per Note ID
    UserVote(u64, Address),    // Jejak vote: (Note ID, Address user) → anti double vote
}

// ================================================
// 2. STRUKTUR DATA (di-upgrade dari Note lama)
// Ditambahkan: score (untuk voting) + author (untuk ownership & keamanan)
// ================================================
#[contracttype]
#[derive(Clone, Debug)]
pub struct Note {
    pub id: u64,
    pub title: String,
    pub content: String,
    pub score: i32,
    pub author: Address,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Comment {
    pub id: u64,
    pub author: Address,
    pub content: String,
}

// ================================================
// 3. KONTRAK UTAMA – NotesContract (sudah di-upgrade ke Forum Web3 style)
// ================================================
#[contract]
pub struct NotesContract;

#[contractimpl]
impl NotesContract {
    // ========================================
    // CREATE NOTE (dengan author + require_auth)
    // ========================================
    pub fn create_note(env: Env, author: Address, title: String, content: String) -> String {
        // Keamanan Web3: pastikan pemilik dompet yang membuat
        author.require_auth();

        let note_id = env.prng().gen::<u64>();

        let note = Note {
            id: note_id,
            title,
            content,
            score: 0,                    // score awal = 0
            author: author.clone(),
        };

        // Simpan Note individu (menggunakan enum key)
        let note_key = DataKey::Note(note_id);
        env.storage().persistent().set(&note_key, &note);

        // Tambahkan ID ke daftar semua notes (master list)
        let all_key = DataKey::AllNotes;
        let mut all_notes: Vec<u64> = env.storage().persistent()
            .get(&all_key)
            .unwrap_or(Vec::new(&env));

        all_notes.push_back(note_id);
        env.storage().persistent().set(&all_key, &all_notes);

        String::from_str(&env, "Note berhasil dibuat!")
    }

    // ========================================
    // GET ALL NOTES (menggunakan master list + fetch per ID)
    // ========================================
    pub fn get_notes(env: Env) -> Vec<Note> {
        let all_key = DataKey::AllNotes;
        let all_ids: Vec<u64> = env.storage().persistent()
            .get(&all_key)
            .unwrap_or(Vec::new(&env));

        let mut notes_vec: Vec<Note> = Vec::new(&env);

        for i in 0..all_ids.len() {
            let id = all_ids.get(i).unwrap();
            let note_key = DataKey::Note(id);
            if let Some(note) = env.storage().persistent().get(&note_key) {
                notes_vec.push_back(note);
            }
        }

        notes_vec
    }

    // ========================================
    // DELETE NOTE (hanya author yang boleh + bersihkan data terkait)
    // ========================================
    pub fn delete_note(env: Env, id: u64, deleter: Address) -> String {
        deleter.require_auth();

        let note_key = DataKey::Note(id);
        let note_opt: Option<Note> = env.storage().persistent().get(&note_key);

        let note = match note_opt {
            Some(n) => n,
            None => return String::from_str(&env, "Note tidak ditemukan"),
        };

        // Cek ownership
        if note.author != deleter {
            panic!("Hanya author yang boleh menghapus note ini!");
        }

        // Hapus Note
        env.storage().persistent().remove(&note_key);

        // Hapus dari master list AllNotes
        let all_key = DataKey::AllNotes;
        let mut all_notes: Vec<u64> = env.storage().persistent()
            .get(&all_key)
            .unwrap_or(Vec::new(&env));

        for i in 0..all_notes.len() {
            if all_notes.get(i).unwrap() == id {
                all_notes.remove(i);
                env.storage().persistent().set(&all_key, &all_notes);
                break;
            }
        }

        // Bersihkan komentar (opsional tapi rapi)
        let comm_key = DataKey::NoteComments(id);
        if env.storage().persistent().has(&comm_key) {
            env.storage().persistent().remove(&comm_key);
        }

        // Votes dibiarkan (tidak perlu dibersihkan, aman)

        String::from_str(&env, "Note berhasil dihapus!")
    }

    // ========================================
    // VOTE (Upvote / Downvote) + ANTI DOUBLE VOTE
    // ========================================
    pub fn vote(env: Env, note_id: u64, voter: Address, is_upvote: bool) {
        voter.require_auth();

        // Cek apakah sudah pernah vote
        let vote_key = DataKey::UserVote(note_id, voter.clone());
        if env.storage().persistent().has(&vote_key) {
            panic!("Kamu sudah memberikan vote pada note ini!");
        }

        // Ambil & update score Note
        let note_key = DataKey::Note(note_id);
        let mut note: Note = env.storage().persistent()
            .get(&note_key)
            .expect("Note tidak ditemukan");

        if is_upvote {
            note.score += 1;
        } else {
            note.score -= 1;
        }

        env.storage().persistent().set(&note_key, &note);

        // Catat bahwa user sudah vote (simpan true)
        env.storage().persistent().set(&vote_key, &true);
    }

    // ========================================
    // ADD COMMENT + relasi ke Note
    // ========================================
    pub fn add_comment(env: Env, note_id: u64, author: Address, content: String) {
        author.require_auth();

        // Pastikan Note ada
        let note_key = DataKey::Note(note_id);
        if !env.storage().persistent().has(&note_key) {
            panic!("Note tidak ditemukan!");
        }

        let comment_key = DataKey::NoteComments(note_id);
        let mut comments: Vec<Comment> = env.storage().persistent()
            .get(&comment_key)
            .unwrap_or(Vec::new(&env));

        let new_comment = Comment {
            id: env.prng().gen::<u64>(),
            author,
            content,
        };

        comments.push_back(new_comment);
        env.storage().persistent().set(&comment_key, &comments);
    }

    // ========================================
    // BONUS: GET COMMENTS untuk satu Note (berguna di frontend)
    // ========================================
    pub fn get_comments(env: Env, note_id: u64) -> Vec<Comment> {
        let comment_key = DataKey::NoteComments(note_id);
        env.storage().persistent()
            .get(&comment_key)
            .unwrap_or(Vec::new(&env))
    }

    // ========================================
    // BONUS: GET SINGLE NOTE (berguna di frontend)
    // ========================================
    pub fn get_note(env: Env, id: u64) -> Option<Note> {
        let note_key = DataKey::Note(id);
        env.storage().persistent().get(&note_key)
    }
}

mod test;