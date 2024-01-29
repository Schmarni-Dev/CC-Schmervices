-- Add migration script here
CREATE TABLE IF NOT EXISTS system ( 
        key INTEGER PRIMARY KEY UNIQUE DEFAULT 0,
        visits INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS users (
        username TEXT NOT NULL PRIMARY KEY UNIQUE,
        display_name TEXT NOT NULL,
        secret TEXT NOT NULL,
        money INTEGER NOT NULL,
        otp_verified INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS auth_tokens (
        token TEXT NOT NULL PRIMARY KEY UNIQUE,
        username TEXT NOT NULL,
        expire_timestamp INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS transactions (
        id TEXT NOT NULL PRIMARY KEY,
        buyer TEXT NOT NULL,
        seller TEXT NOT NULL,
        name TEXT NOT NULL,
        amount INTEGER NOT NULL,
        accepted INTEGER NOT NULL,
        timestamp INTEGER NOT NULL
);

INSERT INTO system VALUES (0,0)
