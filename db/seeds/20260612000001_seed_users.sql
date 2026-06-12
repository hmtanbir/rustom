-- Insert seed users
-- Note: password_digest for these users is 'password' hashed with Argon2id
INSERT INTO users (name, email, password_digest, role, status) VALUES
    ('Admin User', 'admin@example.com', '$argon2id$v=19$m=19456,t=2,p=1$mIk38++6ZCEyzKo+edgXEw$/h0anRjDkzS46suJM6/P3+DySS3qp1+6jXtNjd6UMTs', 0, 1),
ON CONFLICT (email) DO UPDATE SET password_digest = EXCLUDED.password_digest;
