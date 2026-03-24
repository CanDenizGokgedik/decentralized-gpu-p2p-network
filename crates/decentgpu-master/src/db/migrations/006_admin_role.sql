-- Migration 006: add admin role
ALTER TABLE users DROP CONSTRAINT users_role_check;
ALTER TABLE users ADD CONSTRAINT users_role_check CHECK (role IN ('hirer', 'worker', 'both', 'admin'));
