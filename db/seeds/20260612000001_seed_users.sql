-- Insert seed users
-- Note: password_hash for these users is 'password' hashed with Argon2id
INSERT INTO users (email, password_hash, role) VALUES
    ('admin@example.com', '$argon2id$v=19$m=19456,t=2,p=1$mIk38++6ZCEyzKo+edgXEw$/h0anRjDkzS46suJM6/P3+DySS3qp1+6jXtNjd6UMTs', 'admin'),
    ('user@example.com', '$argon2id$v=19$m=19456,t=2,p=1$mIk38++6ZCEyzKo+edgXEw$/h0anRjDkzS46suJM6/P3+DySS3qp1+6jXtNjd6UMTs', 'user')
ON CONFLICT (email) DO UPDATE SET password_hash = EXCLUDED.password_hash;
