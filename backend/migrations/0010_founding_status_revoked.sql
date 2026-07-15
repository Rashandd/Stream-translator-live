-- BH-03C Founding applications use `revoked` as the operator terminal state.
ALTER TABLE founding_applications
    DROP CONSTRAINT IF EXISTS founding_applications_status_check;
ALTER TABLE founding_applications
    ADD CONSTRAINT founding_applications_status_check
    CHECK (status IN ('pending', 'approved', 'revoked'));
