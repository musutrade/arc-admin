import { ChangeDetectionStrategy, Component, inject } from '@angular/core';
import { Router } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';

@Component({
  selector: 'app-error-403',
  imports: [MatIconModule],
  templateUrl: './error-403.html',
  styleUrl: './error-pages.scss',
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class Error403Page {
  private readonly router = inject(Router);

  goHome(): void {
    this.router.navigate(['/permissions']);
  }
}
