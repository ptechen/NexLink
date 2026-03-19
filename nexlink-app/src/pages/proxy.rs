use leptos::prelude::*;

#[component]
pub fn ProxyPage() -> impl IntoView {
    view! {
        <div class="p-3 max-w-4xl mx-auto">
            <div class="mb-4">
                <h1 class="h2 fw-bold text-white mb-2">"Proxy Control"</h1>
                <p class="text-secondary">"Manage your decentralized proxy connections and routing"</p>
            </div>

            <div class="card bg-surface border-border rounded-3 mb-4">
                <div class="card-body">
                    <div class="d-flex align-items-center justify-content-between mb-4">
                        <h2 class="h5 fw-semibold text-white">"Proxy Status"</h2>
                        <div class="d-flex align-items-center gap-3">
                            <span class="badge bg-success bg-opacity-20 text-success">"Active"</span>
                        </div>
                    </div>

                    <div class="row g-4 mb-4">
                        <div class="col-md-4">
                            <div class="bg-slate-800-50 p-3 rounded border border-border">
                                <div class="text-secondary small mb-1">"Active Connections"</div>
                                <div class="h3 fw-bold text-cyber-cyan">"24"</div>
                            </div>
                        </div>
                        <div class="col-md-4">
                            <div class="bg-slate-800-50 p-3 rounded border border-border">
                                <div class="text-secondary small mb-1">"Data Transferred"</div>
                                <div class="h3 fw-bold text-cyber-cyan">"1.2 GB"</div>
                            </div>
                        </div>
                        <div class="col-md-4">
                            <div class="bg-slate-800-50 p-3 rounded border border-border">
                                <div class="text-secondary small mb-1">"Avg. Latency"</div>
                                <div class="h3 fw-bold text-cyber-cyan">"38ms"</div>
                            </div>
                        </div>
                    </div>

                    <div class="d-flex flex-wrap gap-3">
                        <button class="btn btn-primary bg-cyber-cyan border-0 text-dark">
                            "Pause Proxy"
                        </button>
                        <button class="btn btn-outline-secondary">
                            "Route Settings"
                        </button>
                        <button class="btn btn-outline-secondary">
                            "Connection Rules"
                        </button>
                    </div>
                </div>
            </div>

            <div class="card bg-surface border-border rounded-3 mb-4">
                <div class="card-body">
                    <div class="d-flex align-items-center justify-content-between mb-4">
                        <h2 class="h5 fw-semibold text-white">"Routing Rules"</h2>
                        <button class="btn btn-sm btn-outline-secondary">
                            "Add Rule"
                        </button>
                    </div>

                    <div class="d-flex flex-column gap-3">
                        <div class="d-flex align-items-center justify-content-between p-3 bg-slate-800-30 rounded border border-border">
                            <div class="d-flex align-items-center gap-3">
                                <span class="text-white">"Direct"</span>
                                <span class="text-secondary small">"192.168.0.0/16, localhost"</span>
                            </div>
                            <div class="text-secondary small">"System default"</div>
                        </div>

                        <div class="d-flex align-items-center justify-content-between p-3 bg-slate-800-30 rounded border border-border">
                            <div class="d-flex align-items-center gap-3">
                                <span class="text-white">"Proxy"</span>
                                <span class="text-secondary small">"*.google.com, *.youtube.com"</span>
                            </div>
                            <div class="text-secondary small">"P2P Network"</div>
                        </div>

                        <div class="d-flex align-items-center justify-content-between p-3 bg-slate-800-30 rounded border border-border">
                            <div class="d-flex align-items-center gap-3">
                                <span class="text-white">"Block"</span>
                                <span class="text-secondary small">"malicious-domains.list"</span>
                            </div>
                            <div class="text-secondary small">"Blocked"</div>
                        </div>
                    </div>
                </div>
            </div>

            <div class="card bg-surface border-border rounded-3">
                <div class="card-body">
                    <h2 class="h5 fw-semibold text-white mb-4">"Active Connections"</h2>

                    <div class="table-responsive">
                        <table class="table table-borderless text-sm mb-0">
                            <thead>
                                <tr class="border-bottom border-border text-start text-secondary">
                                    <th class="pb-2">"Destination"</th>
                                    <th class="pb-2">"Via Node"</th>
                                    <th class="pb-2">"Type"</th>
                                    <th class="pb-2">"Speed"</th>
                                    <th class="pb-2">"Time"</th>
                                </tr>
                            </thead>
                            <tbody class="text-light border-top-0">
                                <tr>
                                    <td class="py-2">"www.example.com:443"</td>
                                    <td>"Node 3 (Tokyo)"</td>
                                    <td><span class="badge bg-cyber-cyan-20 text-cyber-cyan">"HTTPS"</span></td>
                                    <td>"2.1 MB/s"</td>
                                    <td>"2m 34s"</td>
                                </tr>
                                <tr>
                                    <td class="py-2">"api.service.net:80"</td>
                                    <td>"Node 7 (Frankfurt)"</td>
                                    <td><span class="badge bg-cyber-cyan-20 text-cyber-cyan">"HTTP"</span></td>
                                    <td>"856 KB/s"</td>
                                    <td>"1m 12s"</td>
                                </tr>
                                <tr>
                                    <td class="py-2">"cdn.resource.org:443"</td>
                                    <td>"Node 1 (NYC)"</td>
                                    <td><span class="badge bg-cyber-cyan-20 text-cyber-cyan">"TLS"</span></td>
                                    <td>"3.4 MB/s"</td>
                                    <td>"45s"</td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>
        </div>
    }
}