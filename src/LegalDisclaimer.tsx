import { useState } from "react";
import { motion } from "framer-motion";
import "./LegalDisclaimer.css";

interface LegalDisclaimerProps {
    onAccept: () => void;
}

function LegalDisclaimer({ onAccept }: LegalDisclaimerProps) {
    const [agreed, setAgreed] = useState(false);

    const handleAccept = () => {
        if (agreed) {
            localStorage.setItem("termsAccepted", "true");
            localStorage.setItem("termsAcceptedDate", new Date().toISOString());
            onAccept();
        }
    };

    return (
        <motion.div
            className="legal-overlay"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.4 }}
        >
            <motion.div
                className="legal-container"
                initial={{ scale: 0.95, opacity: 0 }}
                animate={{ scale: 1, opacity: 1 }}
                transition={{ duration: 0.4, delay: 0.1 }}
            >
                {/* Header */}
                <div className="legal-header">
                    <div className="legal-logo">
                        <span className="alpha-symbol">α</span>
                        <span className="logo-text">Alpha Tube</span>
                    </div>
                    <h1>TERMS OF SERVICE & LEGAL DISCLAIMER</h1>
                    <p className="legal-subtitle">Please read carefully before proceeding</p>
                </div>

                {/* Scrollable Content */}
                <div className="legal-content">
                    {/* Section 1: Purpose */}
                    <section className="legal-section">
                        <h2>§1. PURPOSE AND INTENDED USE</h2>
                        <p>
                            Alpha Tube ("the Software") is a personal video management and archival tool designed
                            <strong> exclusively for lawful personal use</strong>. The Software is intended to assist
                            users in downloading and archiving video content that they have the legal right to
                            download, including but not limited to:
                        </p>
                        <ul>
                            <li>Videos personally uploaded by the user</li>
                            <li>Content explicitly marked as downloadable by the content creator</li>
                            <li>Public domain materials</li>
                            <li>Content licensed under Creative Commons or similar permissive licenses</li>
                            <li>Content for which the user has obtained explicit written authorization from the copyright holder</li>
                        </ul>
                    </section>

                    {/* Section 2: YouTube Notice */}
                    <section className="legal-section warning-section">
                        <h2>§2. THIRD-PARTY PLATFORM INTEGRATION NOTICE</h2>
                        <div className="warning-box">
                            <span className="warning-icon">⚠️</span>
                            <p>
                                <strong>IMPORTANT:</strong> The Software utilizes YouTube's platform <em>solely for
                                    demonstration and search query functionality purposes</em>. This integration is provided
                                as a convenience feature and does not constitute an endorsement, partnership, or
                                affiliation with YouTube, Google LLC, or any subsidiary thereof.
                            </p>
                        </div>
                        <p>
                            Users acknowledge that downloading content from YouTube may be subject to YouTube's
                            Terms of Service. It is the user's sole responsibility to review, understand, and
                            comply with all applicable terms of service of any third-party platform accessed
                            through this Software.
                        </p>
                    </section>

                    {/* Section 3: Prohibited Uses */}
                    <section className="legal-section danger-section">
                        <h2>§3. PROHIBITED USES AND COPYRIGHT COMPLIANCE</h2>
                        <div className="danger-box">
                            <span className="danger-icon">🚫</span>
                            <div>
                                <p><strong>THE FOLLOWING USES ARE STRICTLY PROHIBITED:</strong></p>
                                <ul>
                                    <li>Downloading, copying, or distributing copyrighted materials without authorization</li>
                                    <li>Circumventing digital rights management (DRM) or technological protection measures</li>
                                    <li>Commercial exploitation of downloaded content without proper licensing</li>
                                    <li>Sharing, redistributing, or reselling content obtained through this Software</li>
                                    <li>Any use that violates applicable intellectual property laws or regulations</li>
                                </ul>
                            </div>
                        </div>
                        <p>
                            The developers of Alpha Tube <strong>do not encourage, endorse, condone, or support</strong> the
                            use of this Software for any purpose that infringes upon the intellectual property
                            rights of content creators, publishers, or rights holders.
                        </p>
                    </section>

                    {/* Section 4: User Liability */}
                    <section className="legal-section liability-section">
                        <h2>§4. USER LIABILITY AND INDEMNIFICATION</h2>
                        <p>
                            By installing, accessing, or using the Software, <strong>you expressly acknowledge and
                                agree</strong> that:
                        </p>
                        <ol>
                            <li>
                                <strong>Sole Responsibility:</strong> You are solely and exclusively responsible for
                                ensuring that your use of this Software complies with all applicable local, state,
                                national, and international laws and regulations, including but not limited to
                                copyright law, intellectual property law, and digital rights regulations.
                            </li>
                            <li>
                                <strong>Legal Liability:</strong> You personally assume all legal liability, risk,
                                and consequences arising from your use of this Software. The developers shall not
                                be held liable for any claims, damages, or legal actions resulting from your use
                                or misuse of the Software.
                            </li>
                            <li>
                                <strong>Indemnification:</strong> You agree to indemnify, defend, and hold harmless
                                the developers, contributors, and any affiliated parties from and against any and
                                all claims, liabilities, damages, losses, costs, and expenses (including reasonable
                                attorney's fees) arising from your use of the Software or violation of these Terms.
                            </li>
                        </ol>
                    </section>

                    {/* Section 5: No Warranty */}
                    <section className="legal-section">
                        <h2>§5. DISCLAIMER OF WARRANTIES</h2>
                        <p className="all-caps-notice">
                            THE SOFTWARE IS PROVIDED "AS IS" AND "AS AVAILABLE" WITHOUT WARRANTY OF ANY KIND,
                            EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE IMPLIED WARRANTIES OF
                            MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE, AND NON-INFRINGEMENT.
                        </p>
                        <p>
                            The developers make no representations or warranties regarding the accuracy,
                            reliability, completeness, or timeliness of the Software or any content obtained
                            through its use. The developers do not warrant that the Software will be
                            uninterrupted, error-free, or free of harmful components.
                        </p>
                    </section>

                    {/* Section 6: Limitation of Liability */}
                    <section className="legal-section">
                        <h2>§6. LIMITATION OF LIABILITY</h2>
                        <p>
                            IN NO EVENT SHALL THE DEVELOPERS, CONTRIBUTORS, OR ANY AFFILIATED PARTIES BE LIABLE
                            FOR ANY INDIRECT, INCIDENTAL, SPECIAL, CONSEQUENTIAL, OR PUNITIVE DAMAGES, INCLUDING
                            BUT NOT LIMITED TO LOSS OF PROFITS, DATA, USE, GOODWILL, OR OTHER INTANGIBLE LOSSES,
                            ARISING OUT OF OR RELATING TO YOUR USE OF OR INABILITY TO USE THE SOFTWARE.
                        </p>
                    </section>

                    {/* Section 7: Acknowledgment */}
                    <section className="legal-section acknowledgment-section">
                        <h2>§7. ACKNOWLEDGMENT AND ACCEPTANCE</h2>
                        <p>
                            By checking the box below and clicking "I Accept," you acknowledge that you have
                            <strong> read, understood, and agree to be legally bound</strong> by all terms and
                            conditions set forth in this Agreement. You further acknowledge that you are of
                            legal age in your jurisdiction to enter into binding contracts.
                        </p>
                        <p>
                            <em>If you do not agree to these terms, you must not install, access, or use the Software.</em>
                        </p>
                    </section>
                </div>

                {/* Agreement Checkbox */}
                <div className="legal-agreement">
                    <label className="agreement-checkbox">
                        <input
                            type="checkbox"
                            checked={agreed}
                            onChange={(e) => setAgreed(e.target.checked)}
                        />
                        <span className="checkbox-custom"></span>
                        <span className="agreement-text">
                            I have read, understood, and agree to the Terms of Service and Legal Disclaimer.
                            I acknowledge that I am solely responsible for my use of this Software and any
                            consequences arising therefrom.
                        </span>
                    </label>
                </div>

                {/* Accept Button */}
                <div className="legal-actions">
                    <motion.button
                        className={`accept-button ${agreed ? "enabled" : "disabled"}`}
                        onClick={handleAccept}
                        disabled={!agreed}
                        whileHover={agreed ? { scale: 1.02 } : {}}
                        whileTap={agreed ? { scale: 0.98 } : {}}
                    >
                        {agreed ? "I Accept — Enter Alpha Tube" : "Please Read and Accept Terms Above"}
                    </motion.button>
                    <p className="legal-footer">
                        Last updated: January 2026 | Version 1.0
                    </p>
                </div>
            </motion.div>
        </motion.div>
    );
}

export default LegalDisclaimer;
