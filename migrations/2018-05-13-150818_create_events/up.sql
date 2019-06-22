CREATE TABLE events (
    id SERIAL PRIMARY KEY,
    receiver VARCHAR(32) NOT NULL,
    content TEXT NOT NULL,
    author VARCHAR(32) NOT NULL,
    time TIMESTAMP NOT NULL,
    `repeat` BIGINT UNSIGNED NULL
)
