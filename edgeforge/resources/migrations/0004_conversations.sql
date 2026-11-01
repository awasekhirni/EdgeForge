-- 0004_conversations.sql
CREATE TABLE conversations (id TEXT PRIMARY KEY, title TEXT NOT NULL, model_id TEXT NOT NULL REFERENCES models(id) ON DELETE CASCADE, preset TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
CREATE TABLE messages (id TEXT PRIMARY KEY, conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE, role TEXT NOT NULL CHECK (role IN ('system','user','assistant','tool')), content TEXT NOT NULL, tokens_in INTEGER, tokens_out INTEGER, created_at INTEGER NOT NULL);
CREATE INDEX idx_messages_conv ON messages(conversation_id, created_at);

