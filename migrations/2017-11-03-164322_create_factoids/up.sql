CREATE TABLE factoids (
    name VARCHAR(32) NOT NULL,
    idx INTEGER NOT NULL,
    content TEXT NOT NULL,
    author VARCHAR(32) NOT NULL,
    created TIMESTAMP NOT NULL,
    PRIMARY KEY (name, idx)
)
