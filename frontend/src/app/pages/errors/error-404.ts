import { ChangeDetectionStrategy, Component, inject } from '@angular/core';
import { Router } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';

@Component({
  selector: 'app-error-404',
  imports: [MatIconModule],
  templateUrl: './error-404.html',
  styleUrl: './error-pages.scss',
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class Error404Page {
  private readonly router = inject(Router);

  goHome(): void {
    this.router.navigate(['/permissions']);
  }

  goBack(): void {
    window.history.back();
  }
}
